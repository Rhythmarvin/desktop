pub mod runtime;

use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::Mutex;
use uuid::Uuid;

use ora_plugin_protocol::lifecycle::InitializeParams;

use crate::runtime::{PluginProcess, PluginProcessHandle};

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
    bootstrap_path: PathBuf,
}

impl PluginRuntime {
    pub fn new(bun_path: PathBuf, bootstrap_path: PathBuf) -> Self {
        Self {
            processes: Mutex::new(HashMap::new()),
            bun_path,
            bootstrap_path,
        }
    }

    /// Starts a plugin by spawning Bun with the bootstrap entry point.
    pub async fn start(&self, plugin_path: &str) -> Result<StartResult, PluginManagerError> {
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
            &self.bootstrap_path,
            init_params,
        )
        .await
        .map_err(|e| PluginManagerError::Runtime(format!("spawn: {e}")))?;

        let handle = PluginProcessHandle::new(process);
        let result = StartResult {
            instance_id: instance_id.clone(),
            session_id,
            plugin_id: format!("mvp.{}", &instance_id.as_str()[..8]),
            plugin_version: "0.1.0".into(),
        };

        self.processes
            .lock()
            .await
            .insert(instance_id, handle);

        Ok(result)
    }

    /// Sends a JSON-RPC Request to a running plugin and waits for the Response.
    pub async fn invoke(
        &self,
        instance_id: &PluginInstanceId,
        method: &str,
        params: serde_json::Value,
    ) -> Result<InvokeResult, PluginManagerError> {
        let processes = self.processes.lock().await;
        let handle = processes
            .get(instance_id)
            .ok_or_else(|| PluginManagerError::NotFound(instance_id.clone()))?;

        let request_id = format!("h:{}", Uuid::new_v4().simple().to_string()[..8].to_string());
        handle
            .invoke(&request_id, method, params)
            .await
            .map_err(|e| PluginManagerError::Runtime(format!("invoke: {e}")))
    }

    /// Sends $/exit Notification and waits for graceful shutdown.
    pub async fn stop(
        &self,
        instance_id: &PluginInstanceId,
    ) -> Result<(), PluginManagerError> {
        let mut processes = self.processes.lock().await;
        let handle = processes
            .remove(instance_id)
            .ok_or_else(|| PluginManagerError::NotFound(instance_id.clone()))?;

        handle
            .shutdown()
            .await
            .map_err(|e| PluginManagerError::Runtime(format!("stop: {e}")))
    }

    /// Lists running plugin instance IDs.
    pub async fn list(&self) -> Vec<PluginInstanceId> {
        self.processes.lock().await.keys().cloned().collect()
    }
}
