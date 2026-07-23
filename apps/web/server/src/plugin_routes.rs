// plugin_routes.rs — Plugin REST API with scanner support.

use crate::plugin_host::PluginHost;
use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use bytes::Bytes;
use futures_util::stream;
use ora_plugin_manager::{PluginEvent, PluginMetadata};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;

type AppState = Arc<PluginHost>;

pub fn router(host: Arc<PluginHost>) -> Router {
    Router::new()
        .route("/api/plugins", get(list_plugins).post(start_plugin))
        .route("/api/plugins/scan", get(scan_plugins))
        .route("/api/plugins/{id}/invoke", post(invoke))
        .route("/api/plugins/{id}/connect", post(connect_agent))
        .route("/api/plugins/{id}/forward", post(forward_acp))
        .route("/api/plugins/{id}/prompt", post(prompt_stream))
        .route("/api/plugins/{id}/stop", post(stop))
        .with_state(host)
}

#[derive(Deserialize)] struct StartBody { id: String }
#[derive(Deserialize)] struct InvokeBody { method: String, #[serde(default)] params: Option<serde_json::Value> }
#[derive(Deserialize)] struct ConnectBody { #[serde(default)] agent_path: Option<String> }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForwardBody { agent_session_id: String, message: serde_json::Value }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptBody { agent_session_id: String, text: String }

fn err(msg: impl Into<String>) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, msg.into())
}

/// Builds a JSON object for one discovered plugin's public fields.
fn plugin_json(p: &ora_plugin_manager::DiscoveredPlugin) -> serde_json::Value {
    let mut obj = serde_json::json!({
        "id": p.id,
        "displayName": p.display_name,
        "kind": p.kind,
        "version": p.version,
    });
    match &p.metadata {
        PluginMetadata::Agent { cli, display_name, description } => {
            obj["metadata"] = serde_json::json!({
                "type": "agent",
                "cli": cli,
                "displayName": display_name,
                "description": description,
            });
        }
        PluginMetadata::Workbench => {
            obj["metadata"] = serde_json::json!({ "type": "workbench" });
        }
    }
    obj
}

/// Scan the plugins directory for available plugins.
async fn scan_plugins(State(host): State<AppState>) -> impl IntoResponse {
    let h = host.clone();
    let plugins = tokio::task::spawn_blocking(move || h.runtime.scan())
        .await.unwrap_or_default();
    (StatusCode::OK, axum::Json(serde_json::json!({
        "plugins": plugins.iter().map(|p| plugin_json(p)).collect::<Vec<_>>(),
    })))
}

/// Start a plugin by its scan-discovered ID.
async fn start_plugin(
    State(host): State<AppState>,
    axum::Json(body): axum::Json<StartBody>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let h = host.clone(); let id = body.id.clone();
    let result = tokio::task::spawn_blocking(move || h.runtime.start_by_id(&id))
        .await.map_err(|e| err(format!("join: {e}")))?
        .map_err(|e| err(format!("start: {e}")))?;
    Ok((StatusCode::OK, axum::Json(serde_json::json!({
        "instanceId": result.instance_id.as_str(), "sessionId": result.session_id,
        "pluginId": result.plugin_id, "status": "started",
    }))))
}

async fn invoke(
    State(host): State<AppState>, Path(id): Path<String>,
    axum::Json(body): axum::Json<InvokeBody>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let h = host.clone(); let mid = body.method.clone();
    let params = body.params.unwrap_or(serde_json::Value::Null);
    let (target, result) = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let instances = h.runtime.list();
        let t = instances.iter().find(|i| i.as_str().starts_with(&id) || i.as_str() == &id)
            .cloned().ok_or_else(|| format!("plugin not found: {id}"))?;
        let r = h.runtime.invoke(&t, &mid, params).map_err(|e| format!("invoke: {e}"))?;
        Ok((t, r))
    }).await.map_err(|e| err(format!("join: {e}")))?
        .map_err(|e| err(e))?;
    Ok((StatusCode::OK, axum::Json(serde_json::json!({
        "instanceId": target.as_str(), "requestId": result.request_id, "result": result.result,
    }))))
}

async fn stop(
    State(host): State<AppState>, Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let h = host.clone();
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let instances = h.runtime.list();
        let t = instances.iter().find(|i| i.as_str().starts_with(&id) || i.as_str() == &id)
            .cloned().ok_or_else(|| format!("plugin not found: {id}"))?;
        h.runtime.stop(&t).map_err(|e| format!("stop: {e}"))
    }).await.map_err(|e| err(format!("join: {e}")))?
        .map_err(|e| err(e))?;
    Ok((StatusCode::OK, axum::Json(serde_json::json!({"status": "stopped"}))))
}

/// POST /api/plugins/{id}/connect
/// Triggers the agent plugin's `acp/connect` handshake (spawn agent + ACP initialize)
/// and returns capabilities + health status.
async fn connect_agent(
    State(host): State<AppState>,
    Path(id): Path<String>,
    axum::Json(body): axum::Json<ConnectBody>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let h = host.clone();
    let id_for_response = id.clone();
    let params = if let Some(agent_path) = body.agent_path {
        serde_json::json!({ "agentPath": agent_path })
    } else {
        serde_json::json!({})
    };
    let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let instances = h.runtime.list();
        let t = instances.iter().find(|i| i.as_str().starts_with(&id) || i.as_str() == &id)
            .cloned().ok_or_else(|| format!("plugin not found: {id}"))?;
        h.runtime.invoke(&t, "acp/connect", params)
            .map_err(|e| format!("connect: {e}"))
    }).await.map_err(|e| err(format!("join: {e}")))?
        .map_err(|e| err(e))?;
    Ok((StatusCode::OK, axum::Json(serde_json::json!({
        "instanceId": id_for_response,
        "requestId": result.request_id,
        "result": result.result,
    }))))
}

/// POST /api/plugins/{id}/forward
/// Forwards an ACP message to the agent plugin.
///
/// For streaming operations like `session/prompt`, the response is an ACK;
/// subsequent events arrive via `acp/event` Notifications routed through the
/// plugin process reader task.
async fn forward_acp(
    State(host): State<AppState>,
    Path(id): Path<String>,
    axum::Json(body): axum::Json<ForwardBody>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let h = host.clone();
    let id_for_response = id.clone();
    let agent_session_id = body.agent_session_id.clone();
    let message = body.message.clone();
    let params = serde_json::json!({
        "agentSessionId": agent_session_id,
        "message": message,
    });
    let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let instances = h.runtime.list();
        let t = instances.iter().find(|i| i.as_str().starts_with(&id) || i.as_str() == &id)
            .cloned().ok_or_else(|| format!("plugin not found: {id}"))?;
        h.runtime.invoke(&t, "acp/forward", params)
            .map_err(|e| format!("forward: {e}"))
    }).await.map_err(|e| err(format!("join: {e}")))?
        .map_err(|e| err(e))?;
    Ok((StatusCode::OK, axum::Json(serde_json::json!({
        "instanceId": id_for_response,
        "requestId": result.request_id,
        "result": result.result,
    }))))
}

// ── Streaming prompt endpoint ─────────────────────────────────────

/// Tagged frame for the NDJSON stream (matches demo branch pattern).
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StreamFrame {
    Data { data: serde_json::Value },
    Error { error: serde_json::Value },
    End,
}

/// POST /api/plugins/{id}/prompt
///
/// Sends a `session/prompt` (via `acp/forward`) and streams ACP events
/// back as an `application/x-ndjson` HTTP response.
///
/// Request body: `{ "agentSessionId": "...", "text": "say hello" }`
///
/// Response (NDJSON stream):
///   {"type":"data","data":{"type":"session_update","update":{...}}}
///   {"type":"data","data":{"type":"completed","stopReason":"end_turn"}}
///   {"type":"end"}
async fn prompt_stream(
    State(host): State<AppState>,
    Path(id): Path<String>,
    axum::Json(body): axum::Json<PromptBody>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let h = host.clone();
    let agent_session_id = body.agent_session_id.clone();
    let text = body.text.clone();

    // Find the instance
    let instance_id = {
        let instances = h.runtime.list();
        instances.iter()
            .find(|i| i.as_str().starts_with(&id) || i.as_str() == &id)
            .cloned()
            .ok_or_else(|| err(format!("plugin not found: {id}")))?
    };

    // Build the ACP session/prompt message
    let params = serde_json::json!({
        "agentSessionId": agent_session_id,
        "message": {
            "method": "session/prompt",
            "params": {
                "sessionId": agent_session_id,
                "prompt": [{ "type": "text", "text": text }]
            }
        }
    });

    // Start streaming invoke
    let events = h.runtime.invoke_streaming(&instance_id, "acp/forward", params)
        .map_err(|e| err(format!("invoke_streaming: {e}")))?;

    // Use Option<UnboundedReceiver> as unfold state.
    // When channel closes (None), the stream ends.
    let body_stream = stream::unfold(Some(events), |mut state| async move {
        let rx = state.as_mut()?;
        match rx.recv().await {
            Some(PluginEvent::SessionUpdate { update, .. }) => {
                let frame = StreamFrame::Data {
                    data: serde_json::json!({ "type": "session_update", "update": update }),
                };
                let mut bytes = serde_json::to_vec(&frame).unwrap_or_default();
                bytes.push(b'\n');
                Some((Ok::<Bytes, Infallible>(Bytes::from(bytes)), state))
            }
            Some(PluginEvent::Completed { result, .. }) => {
                let mut bytes = serde_json::to_vec(&StreamFrame::Data { data: result }).unwrap_or_default();
                bytes.push(b'\n');
                let end_frame = StreamFrame::End;
                let mut end_bytes = serde_json::to_vec(&end_frame).unwrap_or_default();
                end_bytes.push(b'\n');
                bytes.extend_from_slice(&end_bytes);
                Some((Ok::<Bytes, Infallible>(Bytes::from(bytes)), None))
            }
            Some(PluginEvent::Error { code, message, .. }) => {
                let frame = StreamFrame::Error {
                    error: serde_json::json!({ "code": code, "message": message }),
                };
                let mut bytes = serde_json::to_vec(&frame).unwrap_or_default();
                bytes.push(b'\n');
                Some((Ok::<Bytes, Infallible>(Bytes::from(bytes)), None))
            }
            Some(PluginEvent::PermissionRequest { permission, .. }) => {
                let frame = StreamFrame::Data {
                    data: serde_json::json!({ "type": "permission_request", "permission": permission }),
                };
                let mut bytes = serde_json::to_vec(&frame).unwrap_or_default();
                bytes.push(b'\n');
                Some((Ok::<Bytes, Infallible>(Bytes::from(bytes)), state))
            }
            None => {
                let frame = StreamFrame::End;
                let mut bytes = serde_json::to_vec(&frame).unwrap_or_default();
                bytes.push(b'\n');
                Some((Ok::<Bytes, Infallible>(Bytes::from(bytes)), None))
            }
        }
    });

    let mut response = Response::new(Body::from_stream(body_stream));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/x-ndjson"),
    );
    Ok(response)
}

async fn list_plugins(State(host): State<AppState>) -> impl IntoResponse {
    let h = host.clone();
    let instances = tokio::task::spawn_blocking(move || h.runtime.list())
        .await.unwrap_or_default();
    let running: Vec<_> = instances.iter().map(|i| serde_json::json!({
        "instanceId": i.as_str(), "status": "running",
    })).collect();

    let h2 = host.clone();
    let discovered = tokio::task::spawn_blocking(move || h2.runtime.scan())
        .await.unwrap_or_default();
    let available: Vec<_> = discovered.iter().map(|p| plugin_json(p)).collect();

    (StatusCode::OK, axum::Json(serde_json::json!({
        "running": running,
        "available": available,
    })))
}
