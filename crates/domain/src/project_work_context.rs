use crate::{DomainModelError, ProjectId, ProjectWorkContextId};
use serde::{Deserialize, Serialize};

/// Distinguishes which client surface currently owns a project work context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectWorkContextSurface {
    Web,
    Tauri,
}

impl ProjectWorkContextSurface {
    /// Returns the stable SQLite text representation for this surface value.
    pub fn database_value(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Tauri => "tauri",
        }
    }

    /// Converts a persisted surface string into the strongly typed domain enum.
    pub fn from_database_value(value: &str) -> Result<Self, DomainModelError> {
        match value {
            "web" => Ok(Self::Web),
            "tauri" => Ok(Self::Tauri),
            _ => Err(DomainModelError::InvalidProjectWorkContextSurface(
                value.to_string(),
            )),
        }
    }
}

impl TryFrom<&str> for ProjectWorkContextSurface {
    type Error = DomainModelError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_database_value(value)
    }
}

/// Represents one persisted client window binding to the project it is actively working in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectWorkContext {
    pub id: ProjectWorkContextId,
    pub surface: ProjectWorkContextSurface,
    pub window_id: String,
    pub project_id: ProjectId,
    pub lease_expires_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

impl ProjectWorkContext {
    /// Creates one full project work context snapshot from persistence-managed fields.
    pub fn new(
        id: ProjectWorkContextId,
        surface: ProjectWorkContextSurface,
        window_id: impl Into<String>,
        project_id: ProjectId,
        lease_expires_at: i64,
        created_at: i64,
        updated_at: i64,
    ) -> Self {
        Self {
            id,
            surface,
            window_id: window_id.into(),
            project_id,
            lease_expires_at,
            created_at,
            updated_at,
        }
    }
}
