use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use ora_contracts::{RuntimeLogLevel, RuntimeLogLevelStateResponse, SetRuntimeLogLevelRequest};
use ora_logging::{LogLevel, ora_error};

use crate::app_state::AppState;
use crate::error::WebApiError;

/// Returns the authoritative process-wide Web logging state.
pub async fn get_runtime_log_level(
    State(app_state): State<AppState>,
) -> Result<Json<RuntimeLogLevelStateResponse>, WebApiError> {
    app_state
        .runtime_log_level()
        .state()
        .await
        .map(runtime_state_response)
        .map(Json::from)
        .map_err(|source| WebApiError::internal("failed to read runtime log level", source))
}

/// Replaces and persists the process-wide Web logging level for every connected client.
pub async fn set_runtime_log_level(
    State(app_state): State<AppState>,
    request: Result<Json<SetRuntimeLogLevelRequest>, JsonRejection>,
) -> Result<Json<RuntimeLogLevelStateResponse>, WebApiError> {
    let Json(request) = request.map_err(WebApiError::from)?;
    let requested_level = internal_level(request.level);
    app_state
        .runtime_log_level()
        .set_level(requested_level)
        .await
        .map(runtime_state_response)
        .map(Json::from)
        .map_err(|error| {
            if let Some(rollback_error) = error.rollback_error() {
                ora_error!(
                    message = "runtime log-level rollback failed",
                    error = %rollback_error,
                );
            }
            WebApiError::internal("failed to update runtime log level", error)
        })
}

/// Converts shared manager state into the transport-neutral response contract.
fn runtime_state_response(
    state: ora_runtime_settings::RuntimeLogLevelState,
) -> RuntimeLogLevelStateResponse {
    RuntimeLogLevelStateResponse {
        configured_level: contract_level(state.configured_level),
        effective_level: contract_level(state.effective_level),
        startup_override: state.startup_override.map(contract_level),
    }
}

/// Converts the internal logging vocabulary to its wire-level counterpart exhaustively.
fn contract_level(level: LogLevel) -> RuntimeLogLevel {
    match level {
        LogLevel::Trace => RuntimeLogLevel::Trace,
        LogLevel::Debug => RuntimeLogLevel::Debug,
        LogLevel::Info => RuntimeLogLevel::Info,
        LogLevel::Warn => RuntimeLogLevel::Warn,
        LogLevel::Error => RuntimeLogLevel::Error,
    }
}

/// Converts a validated contract enum into the shared logging vocabulary exhaustively.
fn internal_level(level: RuntimeLogLevel) -> LogLevel {
    match level {
        RuntimeLogLevel::Trace => LogLevel::Trace,
        RuntimeLogLevel::Debug => LogLevel::Debug,
        RuntimeLogLevel::Info => LogLevel::Info,
        RuntimeLogLevel::Warn => LogLevel::Warn,
        RuntimeLogLevel::Error => LogLevel::Error,
    }
}
