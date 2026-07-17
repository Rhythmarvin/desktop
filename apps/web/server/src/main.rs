use ora_logging::{LoggingGuard, init_logging, ora_info, register_gitlancer_logger};
use ora_web_server::config::RuntimeConfig;
use ora_web_server::error::WebBootstrapError;
use ora_web_server::{BackendRuntime, PluginBackendOptions};
use std::path::PathBuf;

/// Boots the web server runtime, initializes shared services, and starts serving HTTP traffic.
#[tokio::main]
async fn main() -> Result<(), WebBootstrapError> {
    let runtime_config = RuntimeConfig::from_env()?;
    let _logging_guard = initialize_logging(runtime_config.logging())?;
    register_gitlancer_logger();
    let options =
        PluginBackendOptions::new(plugin_runtime_resources()?, Vec::new()).without_plugin_routes();
    let runtime = BackendRuntime::start(&runtime_config, options).await?;
    let endpoint = runtime.endpoint();

    ora_info!(
        message = "web server listening",
        host = endpoint.ip().to_string(),
        port = endpoint.port()
    );

    tokio::signal::ctrl_c()
        .await
        .map_err(WebBootstrapError::ShutdownSignal)?;
    runtime.shutdown().await
}

/// Initializes structured logging and returns the guard that owns writer lifetimes.
fn initialize_logging(
    logging_config: &ora_logging::LoggingConfig,
) -> Result<LoggingGuard, WebBootstrapError> {
    init_logging(logging_config.clone()).map_err(WebBootstrapError::LoggingInit)
}

/// Resolves the explicit development runtime resource root without consulting system PATH.
fn plugin_runtime_resources() -> Result<PathBuf, WebBootstrapError> {
    if let Some(path) = std::env::var_os("ORA_PLUGIN_RUNTIME_RESOURCES") {
        return Ok(PathBuf::from(path));
    }
    std::env::current_dir()
        .map(|directory| directory.join("runtime-assets").join("prepared"))
        .map_err(WebBootstrapError::DataDirectoryCreate)
}
