//! Agent plugin runtime facade — public API for starting, stopping, and invoking plugins.
//!
//! This trait is the boundary between the management plane and the runtime plane.
//! Management calls `start`/`stop`; callers use `invoke` to send Agent contract methods.

use super::state::StopReason;

/// Result of an Agent contract invocation.
#[derive(Debug, Clone)]
pub struct InvokeResult {
    /// The JSON-RPC response payload.
    pub response: serde_json::Value,
    /// Stream events received (for streaming methods).
    pub stream_events: Vec<serde_json::Value>,
    /// Whether the method was streaming.
    pub streaming: bool,
}

/// Errors from the runtime facade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    /// Plugin not found in the runtime registry.
    PluginNotFound,
    /// Plugin is not running — call start() first.
    NotRunning,
    /// Plugin is in crash loop — must reset before starting.
    CrashLoop,
    /// Start or stop operation timed out.
    Timeout,
    /// Internal runtime error.
    Internal { message: String },
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PluginNotFound => write!(f, "plugin not found"),
            Self::NotRunning => write!(f, "plugin not running"),
            Self::CrashLoop => write!(f, "plugin in crash loop"),
            Self::Timeout => write!(f, "operation timed out"),
            Self::Internal { message } => write!(f, "internal error: {message}"),
        }
    }
}

/// The public runtime facade for Agent plugins.
///
/// Implementations own generation isolation and never expose the stdio protocol.
/// This is a trait for testability (fake implementations for management-plane testing).
pub trait AgentPluginRuntime {
    /// Start or join the plugin's single-flight generation.
    /// Returns Ok(()) if the plugin is Running (or already Running).
    fn start(&self, plugin_id: &str) -> impl std::future::Future<Output = Result<(), RuntimeError>>;

    /// Stop and reap the complete managed process tree for one plugin.
    /// Blocks until the tree is empty (or timeout).
    fn stop(
        &self,
        plugin_id: &str,
        reason: StopReason,
    ) -> impl std::future::Future<Output = Result<(), RuntimeError>>;

    /// Invoke an Agent contract method on a running plugin.
    /// Returns the JSON-RPC response (success or business error).
    fn invoke(
        &self,
        plugin_id: &str,
        provider_id: &str,
        method: &str,
        params: serde_json::Value,
    ) -> impl std::future::Future<Output = Result<InvokeResult, RuntimeError>>;

    /// Reset the crash loop policy for a plugin.
    fn reset_crash_loop(
        &self,
        plugin_id: &str,
    ) -> impl std::future::Future<Output = Result<(), RuntimeError>>;

    /// Check if a plugin is currently Running.
    fn is_running(&self, plugin_id: &str) -> impl std::future::Future<Output = bool>;
}

/// A fake runtime implementation for testing the management plane.
#[cfg(test)]
pub mod fake {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    pub struct FakeRuntime {
        running: Mutex<HashMap<String, bool>>,
        crash_loops: Mutex<HashMap<String, bool>>,
        invoke_log: Mutex<Vec<(String, String, String)>>,
    }

    impl FakeRuntime {
        pub fn new() -> Self {
            Self {
                running: Mutex::new(HashMap::new()),
                crash_loops: Mutex::new(HashMap::new()),
                invoke_log: Mutex::new(Vec::new()),
            }
        }

        pub fn invoke_log(&self) -> Vec<(String, String, String)> {
            self.invoke_log.lock().unwrap().clone()
        }

        pub fn set_crash_loop(&self, id: &str, looped: bool) {
            self.crash_loops.lock().unwrap().insert(id.to_string(), looped);
        }
    }

    impl AgentPluginRuntime for FakeRuntime {
        async fn start(&self, plugin_id: &str) -> Result<(), RuntimeError> {
            if self.crash_loops.lock().unwrap().get(plugin_id) == Some(&true) {
                return Err(RuntimeError::CrashLoop);
            }
            self.running.lock().unwrap().insert(plugin_id.to_string(), true);
            Ok(())
        }

        async fn stop(&self, plugin_id: &str, _reason: StopReason) -> Result<(), RuntimeError> {
            if !self.running.lock().unwrap().contains_key(plugin_id) {
                return Err(RuntimeError::NotRunning);
            }
            self.running.lock().unwrap().remove(plugin_id);
            Ok(())
        }

        async fn invoke(
            &self,
            plugin_id: &str,
            provider_id: &str,
            method: &str,
            _params: serde_json::Value,
        ) -> Result<InvokeResult, RuntimeError> {
            if !self.running.lock().unwrap().contains_key(plugin_id) {
                return Err(RuntimeError::NotRunning);
            }
            self.invoke_log.lock().unwrap().push((
                plugin_id.to_string(),
                provider_id.to_string(),
                method.to_string(),
            ));
            Ok(InvokeResult {
                response: serde_json::json!({"ok": true}),
                stream_events: vec![],
                streaming: false,
            })
        }

        async fn reset_crash_loop(&self, plugin_id: &str) -> Result<(), RuntimeError> {
            self.crash_loops.lock().unwrap().insert(plugin_id.to_string(), false);
            Ok(())
        }

        async fn is_running(&self, plugin_id: &str) -> bool {
            self.running.lock().unwrap().get(plugin_id) == Some(&true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::FakeRuntime;
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn fake_runtime_start_stop() {
        let runtime = FakeRuntime::new();
        assert!(!runtime.is_running("plugin-a").await);
        runtime.start("plugin-a").await.unwrap();
        assert!(runtime.is_running("plugin-a").await);
        runtime.stop("plugin-a", StopReason::ManualStop).await.unwrap();
        assert!(!runtime.is_running("plugin-a").await);
    }

    #[tokio::test]
    async fn crash_loop_blocks_start() {
        let runtime = FakeRuntime::new();
        runtime.set_crash_loop("broken", true);
        let err = runtime.start("broken").await.unwrap_err();
        assert_eq!(err, RuntimeError::CrashLoop);
    }

    #[tokio::test]
    async fn stop_not_running_errors() {
        let runtime = FakeRuntime::new();
        let err = runtime
            .stop("ghost", StopReason::ManualStop)
            .await
            .unwrap_err();
        assert_eq!(err, RuntimeError::NotRunning);
    }

    #[tokio::test]
    async fn invoke_logs_calls() {
        let runtime = FakeRuntime::new();
        runtime.start("plugin-a").await.unwrap();
        runtime
            .invoke("plugin-a", "agent1", "discoverInstallations", serde_json::json!({}))
            .await
            .unwrap();
        let log = runtime.invoke_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].2, "discoverInstallations");
    }

    #[tokio::test]
    async fn reset_crash_loop_allows_start() {
        let runtime = FakeRuntime::new();
        runtime.set_crash_loop("recovered", true);
        assert!(runtime.start("recovered").await.is_err());
        runtime.reset_crash_loop("recovered").await.unwrap();
        assert!(runtime.start("recovered").await.is_ok());
    }
}
