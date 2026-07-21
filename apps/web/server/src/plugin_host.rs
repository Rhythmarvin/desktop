// plugin_host.rs — Minimal plugin host for MVP.
// Manages the PluginRuntime lifecycle within the web server.

use ora_plugin_manager::{PluginManagerConfig, PluginRuntime};
use std::path::PathBuf;
use std::sync::Arc;

/// Wraps the plugin runtime for the lifetime of the server.
pub struct PluginHost {
    pub runtime: Arc<PluginRuntime>,
}

impl PluginHost {
    pub fn new(bun_path: PathBuf, bootstrap_path: PathBuf, config: PluginManagerConfig) -> Self {
        let runtime = Arc::new(PluginRuntime::new(bun_path, bootstrap_path, config));
        Self { runtime }
    }
}

/// Resolves the bun executable path.
pub fn resolve_bun_path() -> PathBuf {
    std::env::var("ORA_BUN_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("bun"))
}

/// Resolves the bootstrap script path relative to the project root.
pub fn resolve_bootstrap_path() -> PathBuf {
    std::env::var("ORA_BOOTSTRAP_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from("packages/plugin-sdk/src/bootstrap/main.ts")
        })
}
