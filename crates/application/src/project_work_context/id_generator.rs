use crate::project_work_context::ports::ProjectWorkContextIdGenerator;
use ora_domain::ProjectWorkContextId;
use uuid::Uuid;

/// Generates UUID-backed identifiers for new project work context snapshots.
#[derive(Clone, Copy, Debug, Default)]
pub struct UuidProjectWorkContextIdGenerator;

impl UuidProjectWorkContextIdGenerator {
    /// Builds the default UUID-backed project work context id generator.
    pub fn new() -> Self {
        Self
    }
}

impl ProjectWorkContextIdGenerator for UuidProjectWorkContextIdGenerator {
    /// Returns a fresh UUID string wrapped in the domain id newtype.
    fn generate_project_work_context_id(&self) -> ProjectWorkContextId {
        ProjectWorkContextId::new(Uuid::new_v4().to_string())
    }
}
