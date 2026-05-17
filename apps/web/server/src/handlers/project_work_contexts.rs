use crate::app_state::AppState;
use crate::error::WebApiError;
use axum::Json;
use axum::extract::State;
use ora_contracts::{
    OpenProjectWorkContextRequest, OpenProjectWorkContextResponse, RenewProjectWorkContextRequest,
    RenewProjectWorkContextResponse,
};

/// Opens or switches one client window into the requested project.
pub async fn open_project_work_context(
    State(app_state): State<AppState>,
    Json(request): Json<OpenProjectWorkContextRequest>,
) -> Result<Json<OpenProjectWorkContextResponse>, WebApiError> {
    app_state
        .project_work_context_api()
        .open_project_work_context(request)
        .map(Json)
        .map_err(WebApiError::from)
}

/// Renews one existing client window lease through the application layer.
pub async fn renew_project_work_context(
    State(app_state): State<AppState>,
    Json(request): Json<RenewProjectWorkContextRequest>,
) -> Result<Json<RenewProjectWorkContextResponse>, WebApiError> {
    app_state
        .project_work_context_api()
        .renew_project_work_context(request)
        .map(Json)
        .map_err(WebApiError::from)
}
