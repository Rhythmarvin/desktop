// plugin_routes.rs — Minimal REST API for plugin IPC MVP.

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
        .route("/api/plugins/{id}/invoke", post(invoke))
        .route("/api/plugins/{id}/stop", post(stop))
        .with_state(host)
}

#[derive(Deserialize)] struct StartBody { path: String }
#[derive(Deserialize)] struct InvokeBody { method: String, #[serde(default)] params: Option<serde_json::Value> }

async fn start_plugin(
    State(host): State<AppState>,
    axum::Json(body): axum::Json<StartBody>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let h = host.clone(); let p = body.path.clone();
    let result = tokio::task::spawn_blocking(move || h.runtime.start(&p))
        .await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("join: {e}")))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("start: {e}")))?;
    Ok((StatusCode::OK, axum::Json(serde_json::json!({
        "instanceId": result.instance_id.as_str(), "sessionId": result.session_id,
        "pluginId": result.plugin_id, "version": result.plugin_version, "status": "started",
    }))))
}

async fn invoke(
    State(host): State<AppState>, Path(id): Path<String>,
    axum::Json(body): axum::Json<InvokeBody>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let h = host.clone(); let mid = body.method.clone();
    let params = body.params.unwrap_or(serde_json::Value::Null);
    let (target, result) = tokio::task::spawn_blocking(move || {
        let instances = h.runtime.list();
        let t = instances.iter().find(|i| i.as_str().starts_with(&id) || i.as_str() == &id)
            .cloned().ok_or_else(|| format!("plugin not found: {id}"))?;
        let r = h.runtime.invoke(&t, &mid, params).map_err(|e| format!("invoke: {e}"))?;
        Ok::<(_, String), String>((t, r))
    }).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("join: {e}")))??;
    Ok((StatusCode::OK, axum::Json(serde_json::json!({
        "instanceId": target.as_str(), "requestId": result.request_id, "result": result.result,
    }))))
}

async fn stop(
    State(host): State<AppState>, Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let h = host.clone();
    tokio::task::spawn_blocking(move || {
        let instances = h.runtime.list();
        let t = instances.iter().find(|i| i.as_str().starts_with(&id) || i.as_str() == &id)
            .cloned().ok_or_else(|| format!("plugin not found: {id}"))?;
        h.runtime.stop(&t).map_err(|e| format!("stop: {e}"))?;
        Ok::<_, String>(t)
    }).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("join: {e}")))??;
    Ok((StatusCode::OK, axum::Json(serde_json::json!({"status": "stopped"}))))
}

async fn list_plugins(State(host): State<AppState>) -> impl IntoResponse {
    let h = host.clone();
    let instances = tokio::task::spawn_blocking(move || h.runtime.list())
        .await.unwrap_or_default();
    (StatusCode::OK, axum::Json(serde_json::json!({
        "plugins": instances.iter().map(|i| serde_json::json!({
            "instanceId": i.as_str(), "status": "running",
        })).collect::<Vec<_>>(),
    })))
}
