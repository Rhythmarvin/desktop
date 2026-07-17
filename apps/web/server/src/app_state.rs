use crate::plugin_api::security::PluginSecurity;
use crate::plugin_api::{InvocationRegistry, PluginBackend, PluginScopeResolver};
use crate::service::{ProjectApi, ProjectWorkContextApi, SessionApi, TaskApi};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Holds the shared state that HTTP handlers need to serve requests.
#[derive(Clone)]
pub struct AppState {
    project_api: Arc<ProjectApi>,
    project_work_context_api: Arc<ProjectWorkContextApi>,
    task_api: Arc<TaskApi>,
    session_api: Arc<SessionApi>,
    plugin_scope_resolver: PluginScopeResolver,
    ready: Arc<AtomicBool>,
    plugin_backend: Option<Arc<dyn PluginBackend>>,
    plugin_security: Option<PluginSecurity>,
    plugin_invocations: InvocationRegistry,
}

impl AppState {
    /// Creates one shared application state value with readiness disabled until bootstrap completes.
    pub(crate) fn new(
        project_api: Arc<ProjectApi>,
        project_work_context_api: Arc<ProjectWorkContextApi>,
        task_api: Arc<TaskApi>,
        session_api: Arc<SessionApi>,
        plugin_scope_resolver: PluginScopeResolver,
    ) -> Self {
        Self {
            project_api,
            project_work_context_api,
            task_api,
            session_api,
            plugin_scope_resolver,
            ready: Arc::new(AtomicBool::new(false)),
            plugin_backend: None,
            plugin_security: None,
            plugin_invocations: InvocationRegistry::default(),
        }
    }

    /// Installs the authenticated plugin facade only after backend readiness dependencies exist.
    pub(crate) fn with_plugin_backend(
        mut self,
        backend: Arc<dyn PluginBackend>,
        security: PluginSecurity,
    ) -> Self {
        self.plugin_backend = Some(backend);
        self.plugin_security = Some(security);
        self
    }

    pub(crate) fn plugin_backend(&self) -> Option<&Arc<dyn PluginBackend>> {
        self.plugin_backend.as_ref()
    }

    pub(crate) fn plugin_security(&self) -> Option<&PluginSecurity> {
        self.plugin_security.as_ref()
    }

    pub(crate) fn plugin_invocations(&self) -> &InvocationRegistry {
        &self.plugin_invocations
    }

    pub(crate) fn plugin_scope_resolver(&self) -> &PluginScopeResolver {
        &self.plugin_scope_resolver
    }

    /// Returns the shared project API that routes delegate into.
    pub fn project_api(&self) -> &Arc<ProjectApi> {
        &self.project_api
    }

    /// Returns the shared project work context API that routes delegate into.
    pub fn project_work_context_api(&self) -> &Arc<ProjectWorkContextApi> {
        &self.project_work_context_api
    }

    /// Returns the shared task API that routes delegate into.
    pub fn task_api(&self) -> &Arc<TaskApi> {
        &self.task_api
    }

    /// Returns the shared session API that routes delegate into.
    pub fn session_api(&self) -> &Arc<SessionApi> {
        &self.session_api
    }

    /// Starts terminal runtime shutdown for every active session owned by the server.
    pub fn shutdown_terminals(&self) {
        self.session_api.shutdown_terminals();
    }

    /// Marks the runtime as ready after bootstrap finishes successfully.
    pub fn mark_ready(&self) {
        self.ready.store(true, Ordering::SeqCst);
    }

    /// Closes readiness before listener and plugin shutdown begins.
    pub fn mark_unready(&self) {
        self.ready.store(false, Ordering::SeqCst);
    }

    /// Stops every plugin generation through the application adapter if it is installed.
    pub async fn shutdown_plugins(&self) {
        if let Some(backend) = &self.plugin_backend {
            let _ = backend.shutdown().await;
        }
    }

    /// Cancels every authenticated invocation stream before waiting for HTTP connection drain.
    pub(crate) async fn cancel_plugin_invocations(&self) {
        self.plugin_invocations.cancel_all().await;
    }

    /// Reports whether bootstrap has completed successfully for readiness checks.
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }
}
