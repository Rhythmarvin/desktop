use ora_application::{
    ApplicationError, Clock, CreateProjectHandler, DeleteProjectHandler, GetProjectHandler,
    ListProjectsHandler, ProjectIdGenerator, ProjectRepository, ProjectRepositoryError,
    UpdateProjectHandler, UuidProjectIdGenerator,
};
use ora_contracts::{
    CreateProjectRequest, CreateProjectResponse, DeleteProjectRequest, DeleteProjectResponse,
    GetProjectRequest, GetProjectResponse, ListProjectsRequest, ListProjectsResponse,
    UpdateProjectRequest, UpdateProjectResponse,
};
use ora_domain::{Project, ProjectId};
use std::cell::RefCell;
use std::rc::Rc;

/// Groups the transport-facing project entry points for the web adapter.
pub struct WebProjectApi<Repository, IdGenerator, ClockSource> {
    create_project: CreateProjectHandler<Repository, IdGenerator, ClockSource>,
    get_project: GetProjectHandler<Repository>,
    list_projects: ListProjectsHandler<Repository>,
    update_project: UpdateProjectHandler<Repository, ClockSource>,
    delete_project: DeleteProjectHandler<Repository, ClockSource>,
}

impl<Repository, IdGenerator, ClockSource> WebProjectApi<Repository, IdGenerator, ClockSource>
where
    Repository: ProjectRepository + Clone,
    IdGenerator: ProjectIdGenerator + Clone,
    ClockSource: Clock + Clone,
{
    pub fn new(repository: Repository, id_generator: IdGenerator, clock: ClockSource) -> Self {
        Self {
            create_project: CreateProjectHandler::new(
                repository.clone(),
                id_generator,
                clock.clone(),
            ),
            get_project: GetProjectHandler::new(repository.clone()),
            list_projects: ListProjectsHandler::new(repository.clone()),
            update_project: UpdateProjectHandler::new(repository.clone(), clock.clone()),
            delete_project: DeleteProjectHandler::new(repository, clock),
        }
    }

    /// Accepts a create-project contract request and delegates the use case to the application layer.
    pub fn create_project(
        &self,
        request: CreateProjectRequest,
    ) -> Result<CreateProjectResponse, ApplicationError> {
        self.create_project.handle(request)
    }

    /// Accepts a get-project contract request and delegates the use case to the application layer.
    pub fn get_project(
        &self,
        request: GetProjectRequest,
    ) -> Result<GetProjectResponse, ApplicationError> {
        self.get_project.handle(request)
    }

    /// Accepts a list-projects contract request and delegates the use case to the application layer.
    pub fn list_projects(
        &self,
        request: ListProjectsRequest,
    ) -> Result<ListProjectsResponse, ApplicationError> {
        self.list_projects.handle(request)
    }

    /// Accepts an update-project contract request and delegates the use case to the application layer.
    pub fn update_project(
        &self,
        request: UpdateProjectRequest,
    ) -> Result<UpdateProjectResponse, ApplicationError> {
        self.update_project.handle(request)
    }

    /// Accepts a delete-project contract request and delegates the use case to the application layer.
    pub fn delete_project(
        &self,
        request: DeleteProjectRequest,
    ) -> Result<DeleteProjectResponse, ApplicationError> {
        self.delete_project.handle(request)
    }
}

#[derive(Debug, Default)]
struct BootstrapProjectRepository {
    projects: RefCell<Vec<Project>>,
}

impl BootstrapProjectRepository {
    /// Returns the visible project snapshot for one identifier.
    fn find_visible_project(&self, project_id: &ProjectId) -> Option<Project> {
        self.projects
            .borrow()
            .iter()
            .find(|project| project.id == *project_id && !project.audit_fields.is_deleted)
            .cloned()
    }
}

#[derive(Clone, Debug, Default)]
struct BootstrapProjectRepositoryHandle {
    repository: Rc<BootstrapProjectRepository>,
}

impl BootstrapProjectRepositoryHandle {
    /// Creates a shared repository handle that can be cloned across application handlers.
    fn new() -> Self {
        Self {
            repository: Rc::new(BootstrapProjectRepository::default()),
        }
    }
}

impl ProjectRepository for BootstrapProjectRepositoryHandle {
    fn create_project(&self, project: Project) -> Result<Project, ProjectRepositoryError> {
        self.repository.projects.borrow_mut().push(project.clone());
        Ok(project)
    }

    fn find_project(
        &self,
        project_id: &ProjectId,
    ) -> Result<Option<Project>, ProjectRepositoryError> {
        Ok(self.repository.find_visible_project(project_id))
    }

    fn list_projects(&self) -> Result<Vec<Project>, ProjectRepositoryError> {
        Ok(self
            .repository
            .projects
            .borrow()
            .iter()
            .filter(|project| !project.audit_fields.is_deleted)
            .cloned()
            .collect())
    }

    fn update_project(&self, project: Project) -> Result<Project, ProjectRepositoryError> {
        let mut projects = self.repository.projects.borrow_mut();
        if let Some(existing_project) = projects.iter_mut().find(|existing_project| {
            existing_project.id == project.id && !existing_project.audit_fields.is_deleted
        }) {
            *existing_project = project.clone();
            Ok(project)
        } else {
            Err(ProjectRepositoryError::OperationFailed(format!(
                "missing bootstrap project during update: {}",
                project.id
            )))
        }
    }

    fn soft_delete_project(
        &self,
        project_id: &ProjectId,
        deleted_at: i64,
    ) -> Result<bool, ProjectRepositoryError> {
        let mut projects = self.repository.projects.borrow_mut();
        if let Some(project) = projects
            .iter_mut()
            .find(|project| project.id == *project_id && !project.audit_fields.is_deleted)
        {
            project.audit_fields.updated_at = deleted_at;
            project.audit_fields.is_deleted = true;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct BootstrapClock;

impl Clock for BootstrapClock {
    fn now_timestamp_millis(&self) -> i64 {
        0
    }
}

/// Builds the minimal project API wiring so the web adapter can stay transport-focused.
fn build_web_project_api()
-> WebProjectApi<BootstrapProjectRepositoryHandle, UuidProjectIdGenerator, BootstrapClock> {
    WebProjectApi::new(
        BootstrapProjectRepositoryHandle::new(),
        UuidProjectIdGenerator::new(),
        BootstrapClock,
    )
}

/// Boots the web adapter with the application-layer project API wiring.
fn main() {
    let _web_project_api = build_web_project_api();
}
