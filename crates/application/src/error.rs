use crate::ProjectRepositoryError;
use thiserror::Error;

/// Enumerates application-visible failures that adapters must translate for callers.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ApplicationError {
    #[error("project not found: {project_id}")]
    ProjectNotFound { project_id: String },
    #[error("project repository operation failed: {message}")]
    ProjectRepository { message: String },
}

impl ApplicationError {
    /// Maps infrastructure-facing repository failures into stable application errors.
    pub(crate) fn from_project_repository_error(error: ProjectRepositoryError) -> Self {
        match error {
            ProjectRepositoryError::OperationFailed(message) => Self::ProjectRepository { message },
        }
    }
}
