use ora_contracts::{WorkflowRun as ContractRun, WorkflowRunStatus as ContractRunStatus};
use ora_domain::{WorkflowRun, WorkflowRunStatus};

/// Converts a domain run into its public contract representation.
pub(crate) fn map_run(run: WorkflowRun) -> ContractRun {
    ContractRun {
        id: run.id.to_string(),
        workflow_id: run.workflow_id.to_string(),
        snapshot_id: run.snapshot_id.to_string(),
        status: map_run_status(run.status),
        state: run.state,
        input: run.input,
        output: run.output,
        error: run.error,
        payload: run.payload,
        started_at: run.started_at,
        finished_at: run.finished_at,
        created_at: run.audit_fields.created_at,
        updated_at: run.audit_fields.updated_at,
    }
}

/// Translates the internal run status into the transport-facing enum.
fn map_run_status(status: WorkflowRunStatus) -> ContractRunStatus {
    match status {
        WorkflowRunStatus::Pending => ContractRunStatus::Pending,
        WorkflowRunStatus::Running => ContractRunStatus::Running,
        WorkflowRunStatus::Succeeded => ContractRunStatus::Succeeded,
        WorkflowRunStatus::Failed => ContractRunStatus::Failed,
        WorkflowRunStatus::Cancelled => ContractRunStatus::Cancelled,
    }
}
