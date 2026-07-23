pub mod config;
pub mod runtime;
pub mod scanner;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;
use tokio::sync::mpsc;

use ora_plugin_protocol::lifecycle::InitializeParams;

pub use config::PluginManagerConfig;
pub use scanner::{DiscoveredPlugin, scan_plugins};
use crate::runtime::{PluginProcess, PluginProcessHandle};

/// Typed metadata for different plugin capabilities.
///
/// Keeps [`DiscoveredPlugin`] stable while allowing each plugin kind to carry
/// its own strongly-typed configuration without polluting the shared struct with
/// `Option` fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginMetadata {
    /// Plugin that bridges Host ↔ external AI agent via ACP.
    Agent {
        /// CLI executable name (e.g. "opencode", "claude").
        cli: String,
        /// Human-readable display name for the agent.
        display_name: String,
        /// Optional longer description.
        description: Option<String>,
    },
    /// Conventional plugin with request/response handlers.
    Workbench,
}

impl PluginMetadata {
    /// Returns the CLI executable name if this is an Agent plugin.
    pub fn agent_cli(&self) -> Option<&str> {
        match self {
            Self::Agent { cli, .. } => Some(cli),
            Self::Workbench => None,
        }
    }

    /// Returns the display name if this is an Agent plugin.
    pub fn agent_display_name(&self) -> Option<&str> {
        match self {
            Self::Agent { display_name, .. } => Some(display_name),
            Self::Workbench => None,
        }
    }
}

/// Events pushed from a plugin back to the Host during streaming operations.
///
/// Tagged union matching the `acp/event` Notification wire format so the
/// Host-side reader task can route events by `request_id` without inspecting
/// the inner payload.
#[derive(Debug, Clone)]
pub enum PluginEvent {
    /// A streaming `session/update` from the agent.
    SessionUpdate {
        /// ID of the Host request that triggered this stream.
        request_id: String,
        /// Raw payload from the ACP `session/update` notification.
        update: serde_json::Value,
    },
    /// A permission request from the agent.
    PermissionRequest {
        request_id: String,
        permission: serde_json::Value,
    },
    /// The streaming operation completed successfully.
    Completed {
        request_id: String,
        result: serde_json::Value,
    },
    /// The streaming operation failed with an error.
    Error {
        request_id: String,
        code: i32,
        message: String,
    },
}

impl PluginEvent {
    /// Returns the Host request ID that this event belongs to.
    pub fn request_id(&self) -> &str {
        match self {
            Self::SessionUpdate { request_id, .. }
            | Self::PermissionRequest { request_id, .. }
            | Self::Completed { request_id, .. }
            | Self::Error { request_id, .. } => request_id,
        }
    }
}

/// Unique identifier for a running plugin instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PluginInstanceId(String);

impl PluginInstanceId {
    pub fn new_random() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PluginInstanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub use runtime::InvokeResult;

/// Result of starting a plugin.
#[derive(Debug, Clone)]
pub struct StartResult {
    pub instance_id: PluginInstanceId,
    pub session_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
}

#[derive(Debug, thiserror::Error)]
pub enum PluginManagerError {
    #[error("plugin not found: {0}")]
    NotFound(PluginInstanceId),
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Minimal plugin runtime — manages Bun child processes for plugin IPC.
pub struct PluginRuntime {
    processes: Mutex<HashMap<PluginInstanceId, PluginProcessHandle>>,
    bun_path: PathBuf,
    config: PluginManagerConfig,
}

impl PluginRuntime {
    pub fn new(bun_path: PathBuf, _bootstrap_path: PathBuf, config: PluginManagerConfig) -> Self {
        Self {
            processes: Mutex::new(HashMap::new()),
            bun_path,
            config,
        }
    }

    /// Scans `plugins_dir()` for discovered plugins with valid manifests.
    pub fn scan(&self) -> Vec<DiscoveredPlugin> {
        scan_plugins(&self.config)
    }

    /// Starts a plugin by its discovered ID (looked up via scan).
    pub fn start_by_id(&self, plugin_id: &str) -> Result<StartResult, PluginManagerError> {
        let plugins = self.scan();
        let found = plugins
            .iter()
            .find(|p| p.id == plugin_id)
            .ok_or_else(|| PluginManagerError::Runtime(format!("plugin not found: {plugin_id}")))?;

        self.start(&found.entry_path.to_string_lossy())
    }

    /// Initializes a plugin: spawn Bun → $/initialize handshake.
    /// Returns the plugin in "awaitingActivate" phase.
    /// Call `activate()` before sending business requests.
    pub fn initialize(&self, plugin_path: &str) -> Result<StartResult, PluginManagerError> {
        eprintln!("[host] initializing plugin: {plugin_path}");
        let instance_id = PluginInstanceId::new_random();
        let session_id = Uuid::new_v4().to_string();

        let init_params = InitializeParams {
            wire_version: 1,
            host_version: "0.1.0".into(),
            runtime_version: "0.1.0".into(),
            session_id: session_id.clone(),
            plugin: ora_plugin_protocol::lifecycle::PluginIdentity {
                id: format!("mvp.{}", &instance_id.as_str()[..8]),
                version: "0.1.0".into(),
            },
            paths: ora_plugin_protocol::lifecycle::PluginPaths {
                extension_path: plugin_path.to_string(),
                entry_path: plugin_path.to_string(),
                storage_path: format!("./data/{}", instance_id),
            },
        };

        let process = PluginProcess::spawn(
            &self.bun_path,
            &PathBuf::from(plugin_path),
            init_params,
        )
        .map_err(|e| PluginManagerError::Runtime(format!("spawn: {e}")))?;

        let handle = PluginProcessHandle::new(process);
        let result = StartResult {
            instance_id: instance_id.clone(),
            session_id,
            plugin_id: format!("mvp.{}", &instance_id.as_str()[..8]),
            plugin_version: "0.1.0".into(),
        };

        self.processes
            .lock().unwrap()
            .insert(instance_id.clone(), handle);

        eprintln!("[host] plugin initialized: id={instance_id} session={}", result.session_id);
        Ok(result)
    }

    /// Activates an initialized plugin: sends $/activate → plugin enters "running" phase.
    pub fn activate(&self, instance_id: &PluginInstanceId) -> Result<(), PluginManagerError> {
        let mut processes = self.processes.lock().unwrap();
        let handle = processes
            .get_mut(instance_id)
            .ok_or_else(|| PluginManagerError::NotFound(instance_id.clone()))?;
        handle
            .activate()
            .map_err(|e| PluginManagerError::Runtime(format!("activate: {e}")))?;
        eprintln!("[host] plugin activated: id={instance_id}");
        Ok(())
    }

    /// Convenience: spawn Bun → $/initialize → $/activate → Running.
    pub fn start(&self, plugin_path: &str) -> Result<StartResult, PluginManagerError> {
        let result = self.initialize(plugin_path)?;
        self.activate(&result.instance_id)?;
        Ok(result)
    }

    /// Sends a JSON-RPC Request to a running plugin and waits for the Response (blocking).
    pub fn invoke(
        &self,
        instance_id: &PluginInstanceId,
        method: &str,
        params: serde_json::Value,
    ) -> Result<InvokeResult, PluginManagerError> {
        let mut processes = self.processes.lock().unwrap();
        let handle = processes
            .get_mut(instance_id)
            .ok_or_else(|| PluginManagerError::NotFound(instance_id.clone()))?;

        let request_id = format!("h:{}", Uuid::new_v4().simple().to_string()[..8].to_string());
        handle
            .invoke(&request_id, method, params)
            .map_err(|e| PluginManagerError::Runtime(format!("invoke: {e}")))
    }

    /// Sends a JSON-RPC Request and immediately returns a stream of [`PluginEvent`]s.
    ///
    /// The caller receives an `mpsc::Receiver` that yields events as the plugin
    /// pushes `acp/event` Notifications back. The channel is closed when the
    /// plugin sends a final `completed` or `error` event (or when the process exits).
    ///
    /// This is the primary API for long-running operations like ACP `session/prompt`
    /// where the agent streams multiple updates before returning a final result.
    pub fn invoke_streaming(
        &self,
        instance_id: &PluginInstanceId,
        method: &str,
        params: serde_json::Value,
    ) -> Result<mpsc::UnboundedReceiver<PluginEvent>, PluginManagerError> {
        let mut processes = self.processes.lock().unwrap();
        let handle = processes
            .get_mut(instance_id)
            .ok_or_else(|| PluginManagerError::NotFound(instance_id.clone()))?;

        let request_id = format!("h:{}", Uuid::new_v4().simple().to_string()[..8].to_string());
        handle
            .invoke_streaming(&request_id, method, params)
            .map_err(|e| PluginManagerError::Runtime(format!("invoke_streaming: {e}")))
    }

    /// Sends $/exit Notification and waits for graceful shutdown (blocking).
    pub fn stop(
        &self,
        instance_id: &PluginInstanceId,
    ) -> Result<(), PluginManagerError> {
        let mut processes = self.processes.lock().unwrap();
        let handle = processes
            .remove(instance_id)
            .ok_or_else(|| PluginManagerError::NotFound(instance_id.clone()))?;

        handle
            .shutdown()
            .map_err(|e| PluginManagerError::Runtime(format!("stop: {e}")))
    }

    /// Lists running plugin instance IDs.
    pub fn list(&self) -> Vec<PluginInstanceId> {
        self.processes.lock().unwrap().keys().cloned().collect()
    }
}
