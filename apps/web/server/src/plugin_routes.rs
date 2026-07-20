/// Plugin HTTP API.
use crate::error::WebApiError;
use crate::plugin_host::PluginHost;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use ora_plugin_manager::{
    CandidateHandle, ManagementSessionId, PluginManagement, PluginRuntimeInvocation,
    SelectionHandle,
};
use ora_plugin_protocol::{
    AgentConversationId, AgentInstallationId, AgentPageLimit, AgentPrompt,
    AgentProviderId, AgentRequest, AgentScope, ClientRequestId, PluginId,
    CancelConversationRequest, DiscoverInstallationsRequest,
    GetConfigurationSummaryRequest, ListConversationsRequest,
    ListMcpServersRequest, ListSkillsRequest,
    SendMessageRequest, StartConversationRequest,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type Host = Arc<PluginHost>;
type SessionMap = Arc<Mutex<HashMap<String, ManagementSessionId>>>;
type AppState = (Host, SessionMap);

pub fn router(host: Arc<PluginHost>) -> Router {
    let sessions: SessionMap = Arc::new(Mutex::new(HashMap::new()));
    Router::new()
        .route("/api/plugins/selections", post(register_selection))
        .route("/api/plugins", get(list_installed))
        .route("/api/plugins/identify", post(identify))
        .route("/api/plugins/install", post(install))
        .route("/api/plugins/{id}/enable", post(enable))
        .route("/api/plugins/{id}/disable", post(disable))
        .route("/api/plugins/{id}", delete(uninstall))
        .route("/api/plugins/{id}/start", post(start))
        .route("/api/plugins/{id}/stop", post(stop))
        .route("/api/plugins/{id}/invoke", post(invoke))
        .with_state((host, sessions))
}

// ── JSON ────────────────────────────────────────────────────────────

#[derive(Deserialize)] struct SelectionBody { path: String }
#[derive(Deserialize)] struct TokenBody { token: String }
#[derive(Deserialize)] struct StopBody { reason: Option<String> }
#[derive(Deserialize)]
struct InvokeBody {
    method: String,
    #[serde(default)] provider_id: Option<String>,
    #[serde(default)] installation_id: Option<String>,
    #[serde(default)] conversation_id: Option<String>,
    #[serde(default)] prompt: Option<String>,
    #[serde(default)] scope: Option<String>,
    #[serde(default)] limit: Option<u64>,
    #[serde(default)] client_request_id: Option<String>,
}

// ── Helpers ─────────────────────────────────────────────────────────

fn new_session() -> Result<ManagementSessionId, WebApiError> {
    ManagementSessionId::new_random().map_err(|e| WebApiError::bad_request(format!("{e}")))
}

fn parse_pid(s: &str) -> Result<PluginId, WebApiError> {
    PluginId::parse(s).map_err(|e| WebApiError::bad_request(format!("invalid plugin id: {e}")))
}
fn parse_prov(s: &str) -> Result<AgentProviderId, WebApiError> {
    AgentProviderId::parse(s).map_err(|e| WebApiError::bad_request(format!("invalid provider: {e}")))
}
fn parse_inst(s: &str) -> Result<AgentInstallationId, WebApiError> {
    AgentInstallationId::parse(s).map_err(|e| WebApiError::bad_request(format!("invalid installation: {e}")))
}
fn parse_conv(s: &str) -> Result<AgentConversationId, WebApiError> {
    AgentConversationId::parse(s).map_err(|e| WebApiError::bad_request(format!("invalid conversation: {e}")))
}
fn parse_prompt(s: &str) -> Result<AgentPrompt, WebApiError> {
    AgentPrompt::parse(s).map_err(|e| WebApiError::bad_request(format!("invalid prompt: {e}")))
}
fn default_scope(s: Option<String>) -> AgentScope {
    AgentScope::Global {}
}

// ── Handlers ────────────────────────────────────────────────────────

async fn register_selection(
    State((host, sessions)): State<AppState>,
    axum::Json(body): axum::Json<SelectionBody>,
) -> Result<impl IntoResponse, WebApiError> {
    let session = new_session()?;
    let sel = host.management().register_native_selection(session.clone(), &std::path::PathBuf::from(&body.path))
        .map_err(|e| WebApiError::bad_request(format!("{e}")))?;
    let token = sel.selection_handle.as_str().to_string();
    sessions.lock().unwrap().insert(token.clone(), session);
    eprintln!("[plugin-api] registered: {} token={}", body.path, token);
    Ok((StatusCode::OK, axum::Json(serde_json::json!({
        "token": token, "displayName": sel.display_name
    }))))
}

async fn list_installed(
    State((host, _)): State<AppState>,
) -> Result<impl IntoResponse, WebApiError> {
    let snap = host.management().scan_installed().await
        .map_err(|e| WebApiError::bad_request(format!("{e}")))?;
    Ok((StatusCode::OK, axum::Json(serde_json::json!({
        "plugins": snap.entries.iter().map(|e| serde_json::json!({
            "pluginId": e.plugin_id.as_ref().map(|id| id.as_str()),
            "location": e.location.to_string_lossy(),
            "validity": format!("{:?}", e.validity),
        })).collect::<Vec<_>>(),
    }))))
}

async fn identify(
    State((host, sessions)): State<AppState>,
    axum::Json(body): axum::Json<TokenBody>,
) -> Result<impl IntoResponse, WebApiError> {
    let session = sessions.lock().unwrap().remove(&body.token)
        .ok_or_else(|| WebApiError::bad_request("unknown or consumed token"))?;
    let handle = SelectionHandle::from_opaque(&body.token)
        .map_err(|e| WebApiError::bad_request(format!("{e}")))?;
    let ident = host.management().identify(&session, handle).await
        .map_err(|e| WebApiError::bad_request(format!("{e}")))?;
    let new_token = ident.candidate_handle.as_str().to_string();
    sessions.lock().unwrap().insert(new_token.clone(), session);
    eprintln!("[plugin-api] identified: {}", ident.plugin_id.as_str());
    Ok((StatusCode::OK, axum::Json(serde_json::json!({
        "pluginId": ident.plugin_id.as_str(),
        "pluginVersion": ident.plugin_version.as_str(),
        "token": new_token,
    }))))
}

async fn install(
    State((host, sessions)): State<AppState>,
    axum::Json(body): axum::Json<TokenBody>,
) -> Result<impl IntoResponse, WebApiError> {
    let session = sessions.lock().unwrap().remove(&body.token)
        .ok_or_else(|| WebApiError::bad_request("unknown or consumed token"))?;
    let handle = CandidateHandle::from_opaque(&body.token)
        .map_err(|e| WebApiError::bad_request(format!("{e}")))?;
    let installed = host.management().install_authorized_candidate(&session, handle).await
        .map_err(|e| WebApiError::bad_request(format!("{e}")))?;
    eprintln!("[plugin-api] installed: {}", installed.plugin_id.as_str());
    Ok((StatusCode::OK, axum::Json(serde_json::json!({
        "pluginId": installed.plugin_id.as_str(),
        "version": installed.record.plugin_version.as_str(),
    }))))
}

macro_rules! simple_action {
    ($name:ident, $method:ident, $desc:literal) => {
        async fn $name(
            State((host, _)): State<AppState>,
            Path(id): Path<String>,
        ) -> Result<impl IntoResponse, WebApiError> {
            let pid = parse_pid(&id)?;
            host.management().$method(pid.clone()).await
                .map_err(|e| WebApiError::bad_request(format!("{e}")))?;
            eprintln!(concat!("[plugin-api] ", $desc, ": {}"), id);
            Ok((StatusCode::OK, axum::Json(serde_json::json!({"status": $desc, "pluginId": id}))))
        }
    };
}
simple_action!(enable, enable, "enabled");
simple_action!(disable, disable, "disabled");
simple_action!(uninstall, uninstall, "uninstalled");

async fn start(
    State((host, _)): State<AppState>, Path(id): Path<String>,
) -> Result<impl IntoResponse, WebApiError> {
    let pid = parse_pid(&id)?;
    host.runtime().start(&pid).await.map_err(|e| WebApiError::bad_request(format!("{e}")))?;
    eprintln!("[plugin-api] started: {}", id);
    Ok((StatusCode::OK, axum::Json(serde_json::json!({"status": "started", "pluginId": id}))))
}

async fn stop(
    State((host, _)): State<AppState>, Path(id): Path<String>,
    axum::Json(body): axum::Json<StopBody>,
) -> Result<impl IntoResponse, WebApiError> {
    let pid = parse_pid(&id)?;
    let reason = ora_plugin_manager::StopReason::ManualStop;
    host.runtime().stop(&pid, reason).await.map_err(|e| WebApiError::bad_request(format!("{e}")))?;
    eprintln!("[plugin-api] stopped: {}", id);
    Ok((StatusCode::OK, axum::Json(serde_json::json!({"status": "stopped", "pluginId": id}))))
}

async fn invoke(
    State((host, _)): State<AppState>, Path(id): Path<String>,
    axum::Json(body): axum::Json<InvokeBody>,
) -> Result<impl IntoResponse, WebApiError> {
    let pid = parse_pid(&id)?;
    let req = build_request(&body)?;
    eprintln!("[plugin-api] invoke: plugin={} method={}", id, body.method);
    let handle = host.runtime().invoke(&pid, req).await
        .map_err(|e| WebApiError::bad_request(format!("invoke: {e}")))?;
    let request_id = handle.request_id().to_owned();
    eprintln!("[plugin-api] invoke accepted: request_id={}", request_id.as_str());

    // Spawn background task to wait for response (unblocks the HTTP handler)
    let rid = request_id.clone();
    tokio::spawn(async move {
        match handle.finish().await {
            Ok(result) => eprintln!("[plugin-api] invoke result: request_id={} {:?}", rid.as_str(), result),
            Err(e) => eprintln!("[plugin-api] invoke FAILED: request_id={} {:?}", rid.as_str(), e),
        }
    });

    Ok((StatusCode::OK, axum::Json(serde_json::json!({
        "status": "invoked",
        "requestId": request_id.as_str()
    }))))
}

fn build_request(body: &InvokeBody) -> Result<AgentRequest, WebApiError> {
    let prov = body.provider_id.as_deref().unwrap_or("example");
    let provider_id = parse_prov(prov)?;
    let scope = default_scope(body.scope.clone());
    Ok(match body.method.as_str() {
        "discoverInstallations" => AgentRequest::DiscoverInstallations(
            DiscoverInstallationsRequest { provider_id, scope }),
        "getConfigurationSummary" => AgentRequest::GetConfigurationSummary(
            GetConfigurationSummaryRequest {
                provider_id,
                installation_id: parse_inst(body.installation_id.as_deref().unwrap_or("main"))?,
                scope,
            }),
        "listSkills" => AgentRequest::ListSkills(ListSkillsRequest {
            provider_id,
            installation_id: parse_inst(body.installation_id.as_deref().unwrap_or("main"))?,
            scope, cursor: None,
            limit: AgentPageLimit::new(body.limit.unwrap_or(10))
                .map_err(|e| WebApiError::bad_request(format!("{e}")))?,
        }),
        "startConversation" => AgentRequest::StartConversation(StartConversationRequest {
            provider_id,
            installation_id: parse_inst(body.installation_id.as_deref().unwrap_or("main"))?,
            scope,
            client_request_id: ClientRequestId::parse(
                body.client_request_id.as_deref().unwrap_or("00000000-0000-4000-8000-000000000001")
            ).map_err(|e| WebApiError::bad_request(format!("{e}")))?,
            prompt: parse_prompt(body.prompt.as_deref().unwrap_or("hello"))?,
        }),
        "sendMessage" => AgentRequest::SendMessage(SendMessageRequest {
            provider_id,
            installation_id: parse_inst(body.installation_id.as_deref().unwrap_or("main"))?,
            conversation_id: parse_conv(body.conversation_id.as_deref().unwrap_or("conv-1"))?,
            scope,
            client_request_id: ClientRequestId::parse(
                body.client_request_id.as_deref().unwrap_or("00000000-0000-4000-8000-000000000002")
            ).map_err(|e| WebApiError::bad_request(format!("{e}")))?,
            prompt: parse_prompt(body.prompt.as_deref().unwrap_or("hello"))?,
        }),
        "cancelConversation" => AgentRequest::CancelConversation(CancelConversationRequest {
            provider_id,
            installation_id: parse_inst(body.installation_id.as_deref().unwrap_or("main"))?,
            conversation_id: parse_conv(body.conversation_id.as_deref().unwrap_or("conv-1"))?,
            scope,
        }),
        m => return Err(WebApiError::bad_request(format!("unknown method: {m}"))),
    })
}
