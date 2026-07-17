use crate::bootstrap::SystemClock;
use ora_application::{
    ApplicationError, OpenProjectWorkContextHandler, RenewProjectWorkContextHandler,
    UuidProjectWorkContextIdGenerator,
};
use ora_contracts::{
    OpenProjectWorkContextRequest, OpenProjectWorkContextResponse, RenewProjectWorkContextRequest,
    RenewProjectWorkContextResponse,
};
use ora_db::{RepositoryPool, SqliteProjectRepository, SqliteProjectWorkContextRepository};

/// Groups the transport-facing project work context entry points for the web adapter.
pub struct ProjectWorkContextApi {
    open_project_work_context: OpenProjectWorkContextHandler<
        SqliteProjectRepository,
        SqliteProjectWorkContextRepository,
        UuidProjectWorkContextIdGenerator,
        SystemClock,
    >,
    renew_project_work_context:
        RenewProjectWorkContextHandler<SqliteProjectWorkContextRepository, SystemClock>,
}

impl ProjectWorkContextApi {
    /// Builds the project work context transport API from the shared repository pool and clock source.
    pub(crate) fn new(pool: RepositoryPool, clock: SystemClock) -> Self {
        let project_repository = SqliteProjectRepository::new(pool.clone());
        let context_repository = SqliteProjectWorkContextRepository::new(pool);

        Self {
            open_project_work_context: OpenProjectWorkContextHandler::new(
                project_repository,
                context_repository.clone(),
                UuidProjectWorkContextIdGenerator::new(),
                clock,
            ),
            renew_project_work_context: RenewProjectWorkContextHandler::new(
                context_repository,
                clock,
            ),
        }
    }

    /// Accepts an open-or-switch request and delegates the use case to the application layer.
    pub fn open_project_work_context(
        &self,
        request: OpenProjectWorkContextRequest,
    ) -> Result<OpenProjectWorkContextResponse, ApplicationError> {
        self.open_project_work_context.handle(request)
    }

    /// Accepts a lease-renewal request and delegates the use case to the application layer.
    pub fn renew_project_work_context(
        &self,
        request: RenewProjectWorkContextRequest,
    ) -> Result<RenewProjectWorkContextResponse, ApplicationError> {
        self.renew_project_work_context.handle(request)
    }
}
