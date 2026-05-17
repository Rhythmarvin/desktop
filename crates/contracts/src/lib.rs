mod frontend;
mod project;
mod project_work_context;
mod session;
mod task;
mod worktree;

pub use frontend::{
    FrontendEndpoint, FrontendHttpMethod, FrontendPathParam, PROJECT_PATH,
    PROJECT_WORK_CONTEXT_OPEN_PATH, PROJECT_WORK_CONTEXT_RENEW_PATH, PROJECTS_PATH, SESSION_PATH,
    SESSIONS_PATH, TASK_PATH, TASKS_PATH, WORKTREE_PATH, WORKTREES_PATH, frontend_endpoints,
};
pub use project::{
    CreateProjectRequest, CreateProjectResponse, DeleteProjectRequest, DeleteProjectResponse,
    GetProjectRequest, GetProjectResponse, ListProjectsRequest, ListProjectsResponse, Project,
    UpdateProjectRequest, UpdateProjectResponse,
};
pub use project_work_context::{
    OpenProjectWorkContextRequest, OpenProjectWorkContextResponse, ProjectWorkContext,
    ProjectWorkContextSurface, RenewProjectWorkContextRequest, RenewProjectWorkContextResponse,
};
pub use session::{
    CreateSessionRequest, CreateSessionResponse, DeleteSessionRequest, DeleteSessionResponse,
    GetSessionRequest, GetSessionResponse, ListSessionsRequest, ListSessionsResponse, Session,
    SessionStatus, UpdateSessionRequest, UpdateSessionResponse,
};
use std::path::Path;
pub use task::{
    CreateTaskRequest, CreateTaskResponse, DeleteTaskRequest, DeleteTaskResponse, GetTaskRequest,
    GetTaskResponse, ListTasksRequest, ListTasksResponse, Task, TaskStatus, UpdateTaskRequest,
    UpdateTaskResponse,
};
use ts_rs::{Config, ExportError, TS};
pub use worktree::{
    CreateWorktreeRequest, CreateWorktreeResponse, DeleteWorktreeRequest, DeleteWorktreeResponse,
    GetWorktreeRequest, GetWorktreeResponse, ListWorktreesRequest, ListWorktreesResponse,
    UpdateWorktreeRequest, UpdateWorktreeResponse, Worktree, WorktreeActivity,
};

/// Exports every contract DTO family into the shared TypeScript package for frontend consumers.
pub fn export_typescript_bindings_to(
    output_directory: impl AsRef<Path>,
) -> Result<(), ExportError> {
    let config = Config::new().with_out_dir(output_directory.as_ref());

    Project::export(&config)?;
    CreateProjectRequest::export(&config)?;
    CreateProjectResponse::export(&config)?;
    GetProjectRequest::export(&config)?;
    GetProjectResponse::export(&config)?;
    ListProjectsRequest::export(&config)?;
    ListProjectsResponse::export(&config)?;
    UpdateProjectRequest::export(&config)?;
    UpdateProjectResponse::export(&config)?;
    DeleteProjectRequest::export(&config)?;
    DeleteProjectResponse::export(&config)?;
    ProjectWorkContextSurface::export(&config)?;
    ProjectWorkContext::export(&config)?;
    OpenProjectWorkContextRequest::export(&config)?;
    OpenProjectWorkContextResponse::export(&config)?;
    RenewProjectWorkContextRequest::export(&config)?;
    RenewProjectWorkContextResponse::export(&config)?;

    SessionStatus::export(&config)?;
    Session::export(&config)?;
    CreateSessionRequest::export(&config)?;
    CreateSessionResponse::export(&config)?;
    GetSessionRequest::export(&config)?;
    GetSessionResponse::export(&config)?;
    ListSessionsRequest::export(&config)?;
    ListSessionsResponse::export(&config)?;
    UpdateSessionRequest::export(&config)?;
    UpdateSessionResponse::export(&config)?;
    DeleteSessionRequest::export(&config)?;
    DeleteSessionResponse::export(&config)?;

    TaskStatus::export(&config)?;
    Task::export(&config)?;
    CreateTaskRequest::export(&config)?;
    CreateTaskResponse::export(&config)?;
    GetTaskRequest::export(&config)?;
    GetTaskResponse::export(&config)?;
    ListTasksRequest::export(&config)?;
    ListTasksResponse::export(&config)?;
    UpdateTaskRequest::export(&config)?;
    UpdateTaskResponse::export(&config)?;
    DeleteTaskRequest::export(&config)?;
    DeleteTaskResponse::export(&config)?;

    WorktreeActivity::export(&config)?;
    Worktree::export(&config)?;
    CreateWorktreeRequest::export(&config)?;
    CreateWorktreeResponse::export(&config)?;
    GetWorktreeRequest::export(&config)?;
    GetWorktreeResponse::export(&config)?;
    ListWorktreesRequest::export(&config)?;
    ListWorktreesResponse::export(&config)?;
    UpdateWorktreeRequest::export(&config)?;
    UpdateWorktreeResponse::export(&config)?;
    DeleteWorktreeRequest::export(&config)?;
    DeleteWorktreeResponse::export(&config)?;

    Ok(())
}
