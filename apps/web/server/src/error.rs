use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use ora_application::ApplicationError;
use ora_plugin_manager::PluginError;
use serde::Serialize;
use thiserror::Error;

/// Reports bootstrap-time configuration, listener, and logging failures for the web server entry point.
#[derive(Debug, Error)]
pub enum WebBootstrapError {
    #[error("invalid ORA_LOG_LEVEL value `{value}`")]
    InvalidLogLevel { value: String },
    #[error("invalid ORA_LOG_MODE value `{value}`")]
    InvalidLogMode { value: String },
    #[error("invalid ORA_LOG_MAX_DAYS value `{value}`")]
    InvalidLogMaxDays {
        value: String,
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("ORA_DATA_DIR must not be empty")]
    InvalidDatabasePathEmpty,
    #[error("ORA_PROJECT_NAME must not be empty")]
    InvalidProjectNameEmpty,
    #[error("ORA_PROJECT_PATH must not be empty")]
    InvalidProjectPathEmpty,
    #[error("ORA_LOG_MAX_DAYS must be greater than zero")]
    InvalidLogMaxDaysZero,
    #[error("failed to create runtime data directory")]
    DataDirectoryCreate(#[source] std::io::Error),
    #[error("failed to bootstrap SQLite database")]
    DatabaseBootstrap(#[source] ora_db::DatabaseError),
    #[error("failed to reconcile bootstrap project: {message}")]
    ProjectBootstrap { message: String },
    #[error(transparent)]
    LoggingInit(#[from] ora_logging::LoggingInitError),
    #[error("failed to bind HTTP listener")]
    Bind(#[source] std::io::Error),
    #[error("HTTP server exited unexpectedly")]
    Serve(#[source] std::io::Error),
    #[error("plugin backend bootstrap failed")]
    PluginBootstrap(#[source] ora_plugin_manager::PluginError),
    #[error("plugin HTTP security bootstrap failed: {message}")]
    PluginSecurity { message: String },
    #[error("backend server task failed")]
    BackendTask(#[source] tokio::task::JoinError),
    #[error("failed to wait for the process shutdown signal")]
    ShutdownSignal(#[source] std::io::Error),
}

/// Represents one structured error response returned by the HTTP adapter.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ErrorEnvelope {
    error: ErrorPayload,
}

/// Carries the stable machine-readable and human-readable fields for one API failure.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ErrorPayload {
    code: &'static str,
    message: String,
}

/// Centralizes application and transport failures into stable HTTP responses.
pub struct WebApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl WebApiError {
    /// Creates a bad-request API error for malformed transport input.
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: message.into(),
        }
    }

    pub(crate) fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code,
            message: message.into(),
        }
    }

    pub(crate) fn unavailable(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "plugin_backend_unavailable",
            message: message.into(),
        }
    }

    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            message: message.into(),
        }
    }
}

impl From<PluginError> for WebApiError {
    fn from(error: PluginError) -> Self {
        let (status, code) = match error {
            PluginError::NotFound { .. } => (StatusCode::NOT_FOUND, "plugin_not_found"),
            PluginError::AlreadyInstalled { .. } | PluginError::InstallConflict { .. } => {
                (StatusCode::CONFLICT, "plugin_conflict")
            }
            PluginError::InvalidManifest { .. }
            | PluginError::UnsupportedSchemaVersion { .. }
            | PluginError::UnsupportedPackageLayout { .. }
            | PluginError::Incompatible { .. }
            | PluginError::UnsupportedKind { .. }
            | PluginError::SourceChanged { .. }
            | PluginError::SelectionHandleInvalid { .. }
            | PluginError::CandidateHandleInvalid { .. }
            | PluginError::DestructiveConfirmationInvalid
            | PluginError::InvalidLaunchGrant => {
                (StatusCode::BAD_REQUEST, "plugin_request_invalid")
            }
            PluginError::Disabled { .. }
            | PluginError::IntegrityMismatch { .. }
            | PluginError::MissingInstallFiles { .. }
            | PluginError::RecoveryRequired { .. }
            | PluginError::RemovalPending { .. } => (StatusCode::CONFLICT, "plugin_unavailable"),
            PluginError::BackendShuttingDown
            | PluginError::PluginRuntimeUnavailable
            | PluginError::LaunchGrantUnavailable { .. } => (
                StatusCode::SERVICE_UNAVAILABLE,
                "plugin_backend_unavailable",
            ),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "plugin_operation_failed"),
        };
        Self {
            status,
            code,
            message: code.replace('_', " "),
        }
    }
}

impl From<ApplicationError> for WebApiError {
    /// Maps stable application errors into transport-visible HTTP status codes.
    fn from(error: ApplicationError) -> Self {
        match error {
            ApplicationError::ProjectNotFound { project_id } => Self {
                status: StatusCode::NOT_FOUND,
                code: "project_not_found",
                message: format!("project not found: {project_id}"),
            },
            ApplicationError::ProjectRepository { message } => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "project_repository_error",
                message,
            },
            ApplicationError::ProjectOccupied { project_id } => Self {
                status: StatusCode::CONFLICT,
                code: "project_occupied",
                message: format!("project is already occupied: {project_id}"),
            },
            ApplicationError::ProjectWorkContextNotFound { surface, window_id } => Self {
                status: StatusCode::NOT_FOUND,
                code: "project_work_context_not_found",
                message: format!("project work context not found for {surface}/{window_id}"),
            },
            ApplicationError::ProjectWorkContextRepository { message } => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "project_work_context_repository_error",
                message,
            },
            ApplicationError::TaskNotFound { task_id } => Self {
                status: StatusCode::NOT_FOUND,
                code: "task_not_found",
                message: format!("task not found: {task_id}"),
            },
            ApplicationError::TaskRepository { message } => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "task_repository_error",
                message,
            },
            ApplicationError::TaskWorktree { message } => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "task_worktree_error",
                message,
            },
            ApplicationError::WorktreeNotFound { worktree_id } => Self {
                status: StatusCode::NOT_FOUND,
                code: "worktree_not_found",
                message: format!("worktree not found: {worktree_id}"),
            },
            ApplicationError::WorktreeRepository { message } => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "worktree_repository_error",
                message,
            },
            ApplicationError::SessionNotFound { session_id } => Self {
                status: StatusCode::NOT_FOUND,
                code: "session_not_found",
                message: format!("session not found: {session_id}"),
            },
            ApplicationError::SessionRepository { message } => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "session_repository_error",
                message,
            },
            ApplicationError::TerminalStartup { message } => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "terminal_startup_error",
                message,
            },
            ApplicationError::TerminalRuntimeMissing { session_id } => Self {
                status: StatusCode::NOT_FOUND,
                code: "terminal_runtime_missing",
                message: format!("terminal runtime missing for session: {session_id}"),
            },
            ApplicationError::TerminalAlreadyAttached { session_id } => Self {
                status: StatusCode::CONFLICT,
                code: "terminal_already_attached",
                message: format!("terminal already attached for session: {session_id}"),
            },
            ApplicationError::TerminalSessionNotTerminal { session_id } => Self {
                status: StatusCode::CONFLICT,
                code: "terminal_session_not_terminal",
                message: format!("session is not a terminal session: {session_id}"),
            },
            ApplicationError::TerminalSessionStopped { session_id } => Self {
                status: StatusCode::CONFLICT,
                code: "terminal_session_stopped",
                message: format!("terminal session already stopped: {session_id}"),
            },
            ApplicationError::InvalidTerminalRequest { message } => Self {
                status: StatusCode::BAD_REQUEST,
                code: "invalid_terminal_request",
                message,
            },
        }
    }
}

impl From<JsonRejection> for WebApiError {
    /// Maps JSON decoding failures into a stable bad-request API response.
    fn from(error: JsonRejection) -> Self {
        Self::bad_request(error.body_text())
    }
}

impl IntoResponse for WebApiError {
    /// Converts the web adapter error into the HTTP response shape shared by every route.
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorEnvelope {
                error: ErrorPayload {
                    code: self.code,
                    message: self.message,
                },
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::WebApiError;
    use axum::body::to_bytes;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use ora_application::ApplicationError;
    use pretty_assertions::assert_eq;
    use serde_json::{Value, json};

    /// Verifies not-found application errors become stable HTTP 404 payloads.
    #[tokio::test]
    async fn maps_not_found_errors_to_http_404() {
        let response = WebApiError::from(ApplicationError::ProjectNotFound {
            project_id: "project-1".to_string(),
        })
        .into_response();
        let status = response.status();
        let body = response.into_body();
        let bytes = match to_bytes(body, usize::MAX).await {
            Ok(bytes) => bytes,
            Err(error) => panic!("failed to read response body: {error}"),
        };
        let actual = match serde_json::from_slice::<Value>(&bytes) {
            Ok(actual) => actual,
            Err(error) => panic!("failed to decode JSON body: {error}"),
        };

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            actual,
            json!({
                "error": {
                    "code": "project_not_found",
                    "message": "project not found: project-1",
                },
            })
        );
    }

    /// Verifies repository failures become stable HTTP 500 payloads.
    #[tokio::test]
    async fn maps_repository_errors_to_http_500() {
        let response = WebApiError::from(ApplicationError::ProjectRepository {
            message: "write failed".to_string(),
        })
        .into_response();
        let status = response.status();
        let body = response.into_body();
        let bytes = match to_bytes(body, usize::MAX).await {
            Ok(bytes) => bytes,
            Err(error) => panic!("failed to read response body: {error}"),
        };
        let actual = match serde_json::from_slice::<Value>(&bytes) {
            Ok(actual) => actual,
            Err(error) => panic!("failed to decode JSON body: {error}"),
        };

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            actual,
            json!({
                "error": {
                    "code": "project_repository_error",
                    "message": "write failed",
                },
            })
        );
    }

    /// Verifies occupied project errors become stable HTTP 409 payloads.
    #[tokio::test]
    async fn maps_project_occupied_errors_to_http_409() {
        let response = WebApiError::from(ApplicationError::ProjectOccupied {
            project_id: "project-1".to_string(),
        })
        .into_response();
        let status = response.status();
        let body = response.into_body();
        let bytes = match to_bytes(body, usize::MAX).await {
            Ok(bytes) => bytes,
            Err(error) => panic!("failed to read response body: {error}"),
        };
        let actual = match serde_json::from_slice::<Value>(&bytes) {
            Ok(actual) => actual,
            Err(error) => panic!("failed to decode JSON body: {error}"),
        };

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            actual,
            json!({
                "error": {
                    "code": "project_occupied",
                    "message": "project is already occupied: project-1",
                },
            })
        );
    }
}
