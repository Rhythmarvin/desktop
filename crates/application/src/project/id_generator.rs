use crate::project::ports::ProjectIdGenerator;
use ora_domain::ProjectId;
use uuid::Uuid;

/// Generates project identifiers as random UUID v4 values.
#[derive(Clone, Copy, Debug, Default)]
pub struct UuidProjectIdGenerator;

impl UuidProjectIdGenerator {
    /// Creates a UUID-backed project identifier generator.
    pub fn new() -> Self {
        Self
    }
}

impl ProjectIdGenerator for UuidProjectIdGenerator {
    /// Produces a fresh UUID v4 project identifier for create requests.
    fn generate_project_id(&self) -> ProjectId {
        ProjectId::new(Uuid::new_v4().to_string())
    }
}
