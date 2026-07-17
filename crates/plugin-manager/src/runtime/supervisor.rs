//! Plugin runtime supervisor — manages per-plugin actor registry and lifecycle routing.
//!
//! Each plugin gets one `RuntimeActor`. The supervisor routes start/stop/invoke
//! commands to the correct actor and manages the actor lifecycle.

use super::actor::{ActorCommand, ActorConfig, ActorError, RuntimeActor};
use super::state::{RuntimeState, StopReason};
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};

/// Handle for sending commands to a plugin's runtime actor.
#[derive(Clone)]
pub struct ActorHandle {
    pub command_tx: mpsc::Sender<ActorCommand>,
}

impl ActorHandle {
    /// Send a command and wait for the response.
    pub async fn send_command(
        &self,
        cmd: ActorCommand,
    ) -> Result<(), mpsc::error::SendError<ActorCommand>> {
        self.command_tx.send(cmd).await
    }

    /// Start the plugin (or join an in-progress start).
    pub async fn start(&self) -> Result<(), ActorError> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send(ActorCommand::Start { responder: tx })
            .await
            .map_err(|_| ActorError::Internal {
                message: "actor mailbox closed".to_string(),
            })?;
        rx.await.unwrap_or(Err(ActorError::Internal {
            message: "actor dropped".to_string(),
        }))
    }

    /// Stop the plugin gracefully.
    pub async fn stop(&self, reason: StopReason) -> Result<(), ActorError> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send(ActorCommand::Stop {
                reason,
                responder: tx,
            })
            .await
            .map_err(|_| ActorError::Internal {
                message: "actor mailbox closed".to_string(),
            })?;
        rx.await.unwrap_or(Err(ActorError::Internal {
            message: "actor dropped".to_string(),
        }))
    }

    /// Reset the crash loop policy.
    pub async fn reset_crash_loop(&self) -> Result<(), ActorError> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send(ActorCommand::ResetCrashLoop { responder: tx })
            .await
            .map_err(|_| ActorError::Internal {
                message: "actor mailbox closed".to_string(),
            })?;
        rx.await.unwrap_or(Err(ActorError::Internal {
            message: "actor dropped".to_string(),
        }))
    }
}

/// The runtime supervisor — owns the actor registry and routes commands.
pub struct PluginRuntimeSupervisor {
    actors: HashMap<String, ActorHandle>,
    config: ActorConfig,
}

impl PluginRuntimeSupervisor {
    pub fn new(config: ActorConfig) -> Self {
        Self {
            actors: HashMap::new(),
            config,
        }
    }

    /// Register a new plugin actor. Returns the actor handle and event sender.
    pub fn register(
        &mut self,
        plugin_id: String,
    ) -> (ActorHandle, mpsc::Sender<super::actor::ActorEvent>) {
        let (actor, cmd_tx, evt_tx) = RuntimeActor::new(self.config.clone());
        let handle = ActorHandle {
            command_tx: cmd_tx,
        };

        // Spawn the actor task
        let _join_handle = tokio::spawn(async move {
            let mut actor = actor;
            actor.run(|| {
                // Use tokio's clock or real time
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64
            }).await;
        });

        self.actors.insert(plugin_id, handle.clone());
        (handle, evt_tx)
    }

    /// Get the actor handle for a plugin, if registered.
    pub fn get(&self, plugin_id: &str) -> Option<&ActorHandle> {
        self.actors.get(plugin_id)
    }

    /// Remove a plugin from the registry (after cleanup).
    pub fn unregister(&mut self, plugin_id: &str) -> Option<ActorHandle> {
        self.actors.remove(plugin_id)
    }

    /// Returns the number of registered plugin actors.
    pub fn len(&self) -> usize {
        self.actors.len()
    }

    /// Returns true if no actors are registered.
    pub fn is_empty(&self) -> bool {
        self.actors.is_empty()
    }
}
