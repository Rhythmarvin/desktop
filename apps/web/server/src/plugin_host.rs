/// Minimal plugin host — initializes PluginRuntimeHub + PluginManagementService.
///
/// This module only handles plugin crate initialization. The HTTP API surface
/// (install, enable, invoke) is exposed through a separate plugin_api module
/// that the router can optionally merge.

use crate::error::WebBootstrapError;
use ora_plugin_manager::{
    DirectoryRuntimeAssetSource, ManagerLease, PluginManagementService, PluginManagerConfig,
    PluginRuntimeAssets, PluginRuntimeHub, ProcessTreeGenerationLauncher, RuntimeAssetStore,
    SystemAuthorityClock, UnavailableLaunchValueResolver,
};
use ora_plugin_protocol::PluginId;
use ora_process::WindowsJobProcessTreeSpawner;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Owns the plugin runtime and management handles for the lifetime of the server.
pub struct PluginHost {
    pub runtime: PluginRuntimeHub<
        ProcessTreeGenerationLauncher<WindowsJobProcessTreeSpawner>,
        UnavailableLaunchValueResolver,
    >,
    pub management: Arc<
        PluginManagementService<SystemAuthorityClock, PluginRuntimeHub<
            ProcessTreeGenerationLauncher<WindowsJobProcessTreeSpawner>,
            UnavailableLaunchValueResolver,
        >>,
    >,
    _lease: Arc<ManagerLease>,
}

impl PluginHost {
    /// Bootstraps the plugin runtime from prepared runtime assets.
    /// Assets must be prepared with `task prepare-plugin-runtime` first.
    pub async fn start(
        data_dir: &Path,
        runtime_resources: &Path,
    ) -> Result<Self, WebBootstrapError> {
        let manager_config = PluginManagerConfig::new(data_dir);

        // Acquire exclusive lease on data directory
        let lease = Arc::new(
            ManagerLease::acquire(&manager_config)
                .map_err(|e| WebBootstrapError::PluginBootstrap {
                    message: format!("failed to acquire manager lease: {e}"),
                })?,
        );

        // Deploy runtime assets (Bun + bootstrap)
        let source = Arc::new(DirectoryRuntimeAssetSource::new(runtime_resources.to_path_buf()));
        let asset_store = RuntimeAssetStore::new(manager_config.plugin_runtime_dir(), source);
        let asset_lease = asset_store.prepare().await.map_err(|e| {
            WebBootstrapError::PluginBootstrap {
                message: format!("failed to prepare runtime assets: {e}"),
            }
        })?;

        let mut assets = PluginRuntimeAssets::from_runtime_lease(asset_lease)
            .await
            .map_err(|e| WebBootstrapError::PluginBootstrap {
                message: format!("failed to load runtime assets: {e}"),
            })?;

        // Inject required Windows environment variables
        for key in ["SystemRoot", "WINDIR", "TEMP", "TMP"] {
            let value = std::env::var_os(key).ok_or_else(|| {
                WebBootstrapError::PluginBootstrap {
                    message: format!("missing required Windows environment variable: {key}"),
                }
            })?;
            assets = assets.with_environment(key, value);
        }

        // Create process-tree launcher (Windows Job Object)
        let launcher = ProcessTreeGenerationLauncher::new(
            WindowsJobProcessTreeSpawner::new()
                .with_tree_cleanup_timeout(manager_config.deadlines.tree_cleanup),
        );

        // Create runtime hub
        let runtime = PluginRuntimeHub::new(
            manager_config.clone(),
            assets,
            launcher,
            UnavailableLaunchValueResolver,
        );

        // Bootstrap management service
        let management = Arc::new(
            PluginManagementService::bootstrap_with_lease(
                manager_config,
                SystemAuthorityClock::new(),
                runtime.clone(),
                std::collections::BTreeMap::new(),
                Arc::clone(&lease),
            )
            .await
            .map_err(|e| WebBootstrapError::PluginBootstrap {
                message: format!("failed to bootstrap plugin management: {e}"),
            })?,
        );

        // Complete late-bound dependency injection
        runtime
            .bind(
                Arc::clone(&management),
                Arc::new(management.runtime_event_sink()),
            )
            .map_err(|e| WebBootstrapError::PluginBootstrap {
                message: format!("failed to bind runtime: {e}"),
            })?;

        Ok(Self {
            runtime,
            management,
            _lease: lease,
        })
    }

    /// Returns the runtime hub.
    pub fn runtime(&self) -> &PluginRuntimeHub<
        ProcessTreeGenerationLauncher<WindowsJobProcessTreeSpawner>,
        UnavailableLaunchValueResolver,
    > {
        &self.runtime
    }

    /// Returns the management service for use by HTTP handlers.
    pub fn management(&self) -> &Arc<
        PluginManagementService<SystemAuthorityClock, PluginRuntimeHub<
            ProcessTreeGenerationLauncher<WindowsJobProcessTreeSpawner>,
            UnavailableLaunchValueResolver,
        >>,
    > {
        &self.management
    }

    /// Registers a native-picked path as a plugin candidate.
    pub fn register_native_selection(
        &self,
        path: &Path,
    ) -> Result<ora_plugin_manager::CandidateSelection, ora_plugin_manager::PluginError> {
        self.management
            .register_native_selection(
                ora_plugin_manager::ManagementSessionId::new_random()
                    .map_err(|e| ora_plugin_manager::PluginError::Internal { message: e.to_string() })?,
                path,
            )
    }
}

/// Resolves the runtime resources directory from env var or defaults.
pub fn plugin_runtime_resources() -> Result<PathBuf, WebBootstrapError> {
    if let Some(path) = std::env::var_os("ORA_PLUGIN_RUNTIME_RESOURCES") {
        return Ok(PathBuf::from(path));
    }
    std::env::current_dir()
        .map(|d| d.join("runtime-assets").join("prepared"))
        .map_err(WebBootstrapError::DataDirectoryCreate)
}

/// Placeholder — real launch grant handling will be added in a follow-up.
pub fn resolve_native_launch_grant(
    plugin_id: &PluginId,
) -> Result<(), WebBootstrapError> {
    let _ = plugin_id;
    Ok(())
}
