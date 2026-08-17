use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use ora_application::DeveloperMode;
use ora_contracts::{DeveloperModeResponse, SetDeveloperModeRequest};

use crate::app_state::AppState;
use crate::error::WebApiError;

/// Returns the shared developer-mode preference from authoritative persistence.
pub async fn get_developer_mode(
    State(app_state): State<AppState>,
) -> Result<Json<DeveloperModeResponse>, WebApiError> {
    app_state
        .backend()
        .developer_mode()
        .await
        .map(developer_mode_response)
        .map(Json::from)
        .map_err(WebApiError::from)
}

/// Replaces the shared developer-mode preference for both Web and Desktop clients.
pub async fn set_developer_mode(
    State(app_state): State<AppState>,
    request: Result<Json<SetDeveloperModeRequest>, JsonRejection>,
) -> Result<Json<DeveloperModeResponse>, WebApiError> {
    let Json(request) = request.map_err(WebApiError::from)?;
    app_state
        .backend()
        .set_developer_mode(internal_developer_mode(request.enabled))
        .await
        .map(developer_mode_response)
        .map(Json::from)
        .map_err(WebApiError::from)
}

/// Converts the domain enum into the transport-neutral response shape.
fn developer_mode_response(mode: DeveloperMode) -> DeveloperModeResponse {
    DeveloperModeResponse {
        enabled: mode.is_enabled(),
    }
}

/// Converts a wire boolean into the domain enum used by persistence call sites.
fn internal_developer_mode(enabled: bool) -> DeveloperMode {
    if enabled {
        DeveloperMode::Enabled
    } else {
        DeveloperMode::Disabled
    }
}
