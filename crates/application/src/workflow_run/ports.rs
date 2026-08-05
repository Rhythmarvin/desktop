use crate::RepositoryError;
use ora_domain::{
    ProjectId, Task, WorkflowNodeRun, WorkflowRun, WorkflowRunDetail, WorkflowRunId,
    WorkflowRunSummary, Worktree,
};

/// Defines graph-agnostic persistence operations for the workflow-run aggregate.
///
/// The execution engine computes graph-derived inputs and calls these methods; this layer never
/// parses the frozen snapshot graph.
pub trait WorkflowRunRepository {
    /// Persists a new run, its run-task, and its worktree in one atomic transaction.
    ///
    /// Runs are created `Pending` with `current_nodes=[]`. The run row MUST be inserted before the
    /// task row because `tasks.workflow_run_id` is an immediate foreign key on `workflow_runs`.
    fn create_run(
        &self,
        run: WorkflowRun,
        task: Task,
        worktree: Worktree,
    ) -> Result<WorkflowRun, RepositoryError>;

    /// Loads one visible run by identifier.
    fn find_run(&self, run_id: &WorkflowRunId) -> Result<Option<WorkflowRun>, RepositoryError>;

    /// Loads one visible run together with its display name (the task title) and node runs.
    fn get_run_detail(
        &self,
        run_id: &WorkflowRunId,
    ) -> Result<Option<WorkflowRunDetail>, RepositoryError>;

    /// Lists visible run summaries for a project, ordered by creation time.
    fn list_runs_by_project(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<WorkflowRunSummary>, RepositoryError>;

    /// Lists the node-run records of one run in stable ascending order.
    fn list_node_runs(
        &self,
        run_id: &WorkflowRunId,
    ) -> Result<Vec<WorkflowNodeRun>, RepositoryError>;
}

/// Supplies new workflow run identifiers for create use cases.
pub trait WorkflowRunIdGenerator {
    /// Produces the identifier for a newly created workflow run.
    fn generate_run_id(&self) -> WorkflowRunId;
}
