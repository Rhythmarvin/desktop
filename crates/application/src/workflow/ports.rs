use crate::RepositoryError;
use ora_domain::{
    CreatedWorkflow, Workflow, WorkflowDetail, WorkflowId, WorkflowSnapshot, WorkflowSnapshotId,
    WorkflowSummary, WorkflowVersion,
};

/// Describes the outcome of deleting a snapshot while preserving aggregate invariants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteSnapshotResult {
    Deleted(WorkflowSnapshot),
    WorkflowNotFound,
    SnapshotNotFound,
    DraftSnapshot,
    ActiveSnapshot,
}

/// Defines persistence operations for the workflow aggregate.
///
/// Methods represent domain operations rather than individual SQL statements,
/// so create, publish, rollback, and activate each execute within a single
/// repository-managed transaction.
pub trait WorkflowRepository {
    /// Persists a new workflow together with its initial draft in one transaction.
    fn create_workflow(
        &self,
        workflow: Workflow,
        draft: WorkflowSnapshot,
    ) -> Result<CreatedWorkflow, RepositoryError>;

    /// Loads one visible workflow by identifier.
    fn find_workflow(&self, workflow_id: &WorkflowId) -> Result<Option<Workflow>, RepositoryError>;

    /// Loads a workflow together with its draft and currently published snapshot.
    fn get_workflow_detail(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Option<WorkflowDetail>, RepositoryError>;

    /// Lists visible workflows with their published version, ordered by creation time.
    fn list_workflows(&self) -> Result<Vec<WorkflowSummary>, RepositoryError>;

    /// Replaces a visible workflow identified by its stable identifier.
    fn update_workflow(&self, workflow: Workflow) -> Result<Option<Workflow>, RepositoryError>;

    /// Marks a visible workflow deleted and cascades the soft-delete to all its snapshots
    /// within a single transaction.
    fn soft_delete_workflow(
        &self,
        workflow_id: &WorkflowId,
        deleted_at: i64,
    ) -> Result<bool, RepositoryError>;

    /// Loads one visible snapshot by workflow and version string (works for both `"draft"`
    /// and published version identifiers).
    fn find_snapshot_by_version(
        &self,
        workflow_id: &WorkflowId,
        version: &str,
    ) -> Result<Option<WorkflowSnapshot>, RepositoryError>;

    /// Lists published (non-draft, non-deleted) version summaries for a workflow,
    /// ordered by creation time descending.
    fn list_versions(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<WorkflowVersion>, RepositoryError>;

    /// Updates the graph of a workflow's draft snapshot in-place.
    fn update_draft(
        &self,
        workflow_id: &WorkflowId,
        graph: String,
        updated_at: i64,
    ) -> Result<Option<WorkflowSnapshot>, RepositoryError>;

    /// Publishes the current draft as an immutable snapshot and activates it
    /// (sets `published_snapshot_id`) within a single transaction.
    fn publish_snapshot(
        &self,
        workflow_id: &WorkflowId,
        snapshot: WorkflowSnapshot,
    ) -> Result<Option<WorkflowSnapshot>, RepositoryError>;

    /// Copies the graph from a historical snapshot into the draft without changing
    /// the published version pointer.
    fn rollback_draft(
        &self,
        workflow_id: &WorkflowId,
        snapshot_id: &WorkflowSnapshotId,
        updated_at: i64,
    ) -> Result<Option<WorkflowSnapshot>, RepositoryError>;

    /// Switches the published version pointer to a different snapshot and syncs its
    /// graph into the draft within a single transaction.
    fn activate_version(
        &self,
        workflow_id: &WorkflowId,
        snapshot_id: &WorkflowSnapshotId,
        updated_at: i64,
    ) -> Result<Option<WorkflowSnapshot>, RepositoryError>;

    /// Marks a visible non-draft, non-active snapshot deleted.
    fn soft_delete_snapshot(
        &self,
        workflow_id: &WorkflowId,
        snapshot_id: &WorkflowSnapshotId,
        deleted_at: i64,
    ) -> Result<DeleteSnapshotResult, RepositoryError>;
}

/// Supplies new workflow and snapshot identifiers for create use cases.
pub trait WorkflowIdGenerator {
    /// Produces the identifier for a newly created workflow.
    fn generate_workflow_id(&self) -> WorkflowId;

    /// Produces the identifier for a newly created snapshot.
    fn generate_snapshot_id(&self) -> WorkflowSnapshotId;
}
