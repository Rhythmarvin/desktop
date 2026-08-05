use ora_contracts::{
    Task as ContractTask, TaskStatus as ContractTaskStatus, TaskType as ContractTaskType,
    TaskWorkspaceMode,
};
use ora_domain::{Task as DomainTask, TaskStatus as DomainTaskStatus, TaskType as DomainTaskType};

/// Maps a domain task into the app-facing contract shape.
pub(crate) fn map_task(task: DomainTask) -> ContractTask {
    let workspace_mode = match task.worktree_id {
        Some(_) => TaskWorkspaceMode::Worktree,
        None => TaskWorkspaceMode::ProjectRoot,
    };

    ContractTask {
        id: task.id.to_string(),
        project_id: task.project_id.to_string(),
        title: task.title,
        status: map_task_status(task.status),
        workspace_mode,
        task_type: map_task_type(task.task_type),
        workflow_run_id: task.workflow_run_id.map(|id| id.to_string()),
    }
}

/// Translates the internal task status into the transport-facing enum.
fn map_task_status(status: DomainTaskStatus) -> ContractTaskStatus {
    match status {
        DomainTaskStatus::Todo => ContractTaskStatus::Todo,
        DomainTaskStatus::Doing => ContractTaskStatus::Doing,
        DomainTaskStatus::Done => ContractTaskStatus::Done,
    }
}

/// Translates the internal task type into the transport-facing enum.
fn map_task_type(task_type: DomainTaskType) -> ContractTaskType {
    match task_type {
        DomainTaskType::Default => ContractTaskType::Default,
        DomainTaskType::WorkflowRun => ContractTaskType::WorkflowRun,
    }
}
