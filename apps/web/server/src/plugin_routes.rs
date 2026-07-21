// plugin_routes.rs — Plugin REST API with scanner support.

use crate::plugin_host::PluginHost;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use serde::Deserialize;
use std::sync::Arc;

type AppState = Arc<PluginHost>;

pub fn router(host: Arc<PluginHost>) -> Router {
    Router::new()
        .route("/api/plugins", get(list_plugins).post(start_plugin))
        .route("/api/plugins/scan", get(scan_plugins))
        .route("/api/plugins/{id}/invoke", post(invoke))
        .route("/api/plugins/{id}/stop", post(stop))
        .with_state(host)
}

#[derive(Deserialize)] struct StartBody { id: String }
#[derive(Deserialize)] struct InvokeBody { method: String, #[serde(default)] params: Option<serde_json::Value> }

fn err(msg: impl Into<String>) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, msg.into())
}

/// Scan the plugins directory for available plugins.
async fn scan_plugins(State(host): State<AppState>) -> impl IntoResponse {
    let h = host.clone();
    let plugins = tokio::task::spawn_blocking(move || h.runtime.scan())
        .await.unwrap_or_default();
    (StatusCode::OK, axum::Json(serde_json::json!({
        "plugins": plugins.iter().map(|p| serde_json::json!({
            "id": p.id,
            "displayName": p.display_name,
            "kind": p.kind,
            "version": p.version,
        })).collect::<Vec<_>>(),
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
    let available: Vec<_> = discovered.iter().map(|p| serde_json::json!({
        "id": p.id, "displayName": p.display_name, "kind": p.kind, "version": p.version,
    })).collect();

    (StatusCode::OK, axum::Json(serde_json::json!({
        "running": running,
        "available": available,
    })))
}
