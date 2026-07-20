mod app_state;
mod bootstrap;
mod config;
mod error;
mod handlers;
mod plugin_host;
mod plugin_routes;
mod routes;
mod service;

use crate::bootstrap::build_app_state;
use crate::config::RuntimeConfig;
use crate::error::WebBootstrapError;
use crate::plugin_host::PluginHost;
use axum::Router;
use std::sync::Arc;
use ora_logging::{LoggingGuard, init_logging, ora_info, register_gitlancer_logger};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), WebBootstrapError> {
    let runtime_config = RuntimeConfig::from_env()?;
    let _logging_guard = initialize_logging(runtime_config.logging())?;
    register_gitlancer_logger();

    let plugin_host = start_plugin_host(&runtime_config).await;

    let app_state = build_app_state(&runtime_config)?;
    let mut router = build_router(app_state.clone());
    if let Some(host) = &plugin_host {
        router = router.merge(plugin_routes::router(host.clone()));
    }
    let listener = bind_listener(&runtime_config).await?;

    app_state.mark_ready();

    ora_info!(
        message = "web server listening",
        host = runtime_config.server().host().to_string(),
        port = runtime_config.server().port()
    );

    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            wait_for_shutdown().await;
            drop(plugin_host);
        })
        .await
        .map_err(WebBootstrapError::Serve)
}

async fn start_plugin_host(runtime_config: &RuntimeConfig) -> Option<Arc<PluginHost>> {
    match plugin_host::plugin_runtime_resources() {
        Ok(resources) if resources.join("runtime-manifest.json").exists() => {
            let data_dir = runtime_config.database().path().parent()
                .unwrap_or_else(|| std::path::Path::new("."));
            match PluginHost::start(data_dir, &resources).await {
                Ok(host) => {
                    ora_info!(message = "plugin host started", resources = %resources.display());
                    Some(Arc::new(host))
                }
                Err(err) => {
                    ora_logging::ora_warn!(message = "plugin host failed to start", error = %err);
                    None
                }
            }
        }
        Ok(resources) => {
            ora_logging::ora_warn!(
                message = "plugin runtime not prepared — run task prepare-plugin-runtime",
                expected = %resources.display()
            );
            None
        }
        Err(_) => None,
    }
}

/// Builds the HTTP router for the configured application state.
fn build_router(app_state: app_state::AppState) -> Router {
    routes::build_router(app_state)
}

/// Binds the Tokio listener using the configured socket address.
async fn bind_listener(runtime_config: &RuntimeConfig) -> Result<TcpListener, WebBootstrapError> {
    TcpListener::bind(runtime_config.server().socket_address())
        .await
        .map_err(WebBootstrapError::Bind)
}

/// Initializes structured logging and returns the guard that owns writer lifetimes.
fn initialize_logging(
    logging_config: &ora_logging::LoggingConfig,
) -> Result<LoggingGuard, WebBootstrapError> {
    init_logging(logging_config.clone()).map_err(WebBootstrapError::LoggingInit)
}

/// Waits for the process shutdown signal so the server stops cleanly on SIGINT.
async fn wait_for_shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}
