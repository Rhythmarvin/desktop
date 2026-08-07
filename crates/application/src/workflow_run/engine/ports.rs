use crate::RepositoryError;
use crate::workflow_run::engine::graph::WorkflowGraph;
use ora_domain::{
    SessionId, Task, WorkflowNodeRun, WorkflowNodeRunId, WorkflowRun, WorkflowRunId, Worktree,
};
use thiserror::Error;

/// A node-run the engine wants to start in one scheduling wave.
///
/// The engine assigns the node-run id; the repository persists the row and the `current_nodes`
/// anchor in the same transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRunToStart {
    pub id: WorkflowNodeRunId,
    pub node_id: String,
    pub node_type: String,
    pub input: Option<String>,
}

/// Everything the engine needs to start or drive a run, fetched in one read.
///
/// `graph_json` is the raw frozen React Flow document; the engine parses it into a `WorkflowGraph`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionContext {
    pub run: WorkflowRun,
    pub task: Task,
    pub worktree: Worktree,
    pub graph_json: String,
}

/// Supplies new node-run identifiers for the engine's scheduling waves.
pub trait WorkflowNodeRunIdGenerator {
    /// Produces the identifier for a newly created node run.
    fn generate_node_run_id(&self) -> WorkflowNodeRunId;
}

/// Failures raised while validating start-time hard prerequisites.
#[derive(Debug, Error)]
pub enum StartPrerequisitesError {
    #[error("workflow skill not found: {skill_id}")]
    WorkflowSkillNotFound { skill_id: String },
    #[error("workflow role not found: {role_id}")]
    WorkflowRoleNotFound { role_id: String },
    #[error("skill materialization failed: {message}")]
    SkillMaterializationError { message: String },
    #[error("repository operation failed")]
    Repository(#[from] RepositoryError),
}

/// Validates start-time hard prerequisites and materializes enabled skills.
///
/// Skills and roles are launch dependencies: every enabled skill must resolve in the catalog and
/// every agent's role must resolve in the agents catalog. The backend implementation also copies
/// the enabled skills into the run worktree's `.claude/skills/` directory before the run starts.
pub trait WorkflowRunStartPrerequisites: Send + Sync {
    /// Resolves every enabled skill and role in the graph and materializes the enabled skills.
    fn validate_and_materialize(
        &self,
        context: &ExecutionContext,
        graph: &WorkflowGraph,
    ) -> Result<(), StartPrerequisitesError>;
}

/// Outcome of starting a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartWorkflowRunResult {
    /// The run transitioned from `Pending` (empty `current_nodes`) to `Running`.
    Started,
    /// The run is not startable; the caller returns the current run idempotently.
    Current,
    NotFound,
}

/// Outcome of advancing one node-run (`complete` or `fail`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvanceWorkflowRunResult {
    /// The node-run transitioned and the run state was maintained.
    Advanced,
    /// The node-run is not `Running` (a late or duplicate callback); the transition is a no-op.
    NotRunning,
    NotFound,
}

/// Outcome of cancelling a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelWorkflowRunResult {
    Cancelled,
    NotActive,
    NotFound,
}

/// Outcome of restarting a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartWorkflowRunResult {
    Restarted,
    NotRestartable,
    NotFound,
}

/// Persistence operations for the workflow run execution engine.
///
/// This port is deliberately separate from the graph-agnostic `WorkflowRunRepository` CRUD port:
/// the engine owns node-run writes and the run state machine, and every state transition must be
/// a single immediate transaction that maintains `state.current_nodes`. No generic overwrite of
/// the full run state is exposed to callers.
pub trait WorkflowRunEngineRepository {
    /// Loads the run, its task, its worktree, and the frozen snapshot graph in one read.
    fn find_execution_context(
        &self,
        run_id: &WorkflowRunId,
    ) -> Result<Option<ExecutionContext>, RepositoryError>;

    /// Lists the node-run rows of one run so the engine can recompute ready and in-flight sets.
    fn list_node_runs(
        &self,
        run_id: &WorkflowRunId,
    ) -> Result<Vec<WorkflowNodeRun>, RepositoryError>;

    /// Binds a node run to its real Ora session right after `attach_session` succeeds, so the
    /// frontend can subscribe to the session's permission stream before the prompt starts.
    fn set_node_run_session_id(
        &self,
        node_run_id: &WorkflowNodeRunId,
        session_id: &SessionId,
        now: i64,
    ) -> Result<(), RepositoryError>;

    /// Starts a run by creating the start node-run and transitioning the run to `Running`.
    ///
    /// Only a `Pending` run with empty `current_nodes` transitions; anything else returns
    /// `Current` so callers can return the existing run idempotently.
    fn start_run(
        &self,
        run_id: &WorkflowRunId,
        start_node_run: &NodeRunToStart,
        now: i64,
    ) -> Result<StartWorkflowRunResult, RepositoryError>;

    /// Starts a wave of ready nodes, creating their node-run rows and updating `current_nodes`.
    fn start_ready_nodes(
        &self,
        run_id: &WorkflowRunId,
        node_runs: &[NodeRunToStart],
        now: i64,
    ) -> Result<(), RepositoryError>;

    /// Marks one node-run succeeded, records its output and stop reason, and removes it from
    /// `current_nodes`.
    fn complete_node(
        &self,
        node_run_id: &WorkflowNodeRunId,
        output: Option<String>,
        stop_reason: Option<String>,
        now: i64,
    ) -> Result<AdvanceWorkflowRunResult, RepositoryError>;

    /// Marks one node-run and its run failed, anchoring the failed node in `current_nodes`.
    fn fail_node(
        &self,
        node_run_id: &WorkflowNodeRunId,
        error: String,
        output: Option<String>,
        now: i64,
    ) -> Result<AdvanceWorkflowRunResult, RepositoryError>;

    /// Finishes a run as succeeded with the given output.
    fn finish_run(
        &self,
        run_id: &WorkflowRunId,
        output: Option<String>,
        now: i64,
    ) -> Result<(), RepositoryError>;

    /// Cancels a running run: the run and its non-terminal node runs become `Cancelled` and
    /// `current_nodes` is cleared.
    fn cancel_run(
        &self,
        run_id: &WorkflowRunId,
        now: i64,
    ) -> Result<CancelWorkflowRunResult, RepositoryError>;

    /// Restarts a non-running run: soft-deletes its node runs and resets it to `Pending` with
    /// empty `current_nodes`, so prior node-run history stays queryable.
    fn restart_run(
        &self,
        run_id: &WorkflowRunId,
        now: i64,
    ) -> Result<RestartWorkflowRunResult, RepositoryError>;

    /// Lists runs in `Running` or `Failed` status for boot-time crash recovery.
    fn list_recoverable_runs(&self) -> Result<Vec<WorkflowRunId>, RepositoryError>;

    /// Fails the non-terminal node runs of the given runs and stops their running sessions.
    fn fail_orphaned_node_runs(
        &self,
        run_ids: &[WorkflowRunId],
        now: i64,
    ) -> Result<(), RepositoryError>;
}
