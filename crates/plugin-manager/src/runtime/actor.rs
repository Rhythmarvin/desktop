//! Runtime actor — owns the state machine, mailbox, and generation guard for one plugin.
//!
//! Commands, frame events, and watcher events all enter the same bounded mailbox.
//! The actor processes them sequentially, ensuring all state transitions are atomic.

use super::state::{
    DrainTrigger, DrainProgress, Generation, Pid, ProcessExit, RuntimeState, SpawnToken,
    StopReason, WriterFailureStage, validate_transition,
};
use tokio::sync::{mpsc, oneshot};

/// Commands that can be sent to the runtime actor.
#[derive(Debug)]
pub enum ActorCommand {
    /// Request to start (or join an in-progress start) the plugin.
    Start {
        responder: oneshot::Sender<Result<(), ActorError>>,
    },

    /// Request to stop the plugin gracefully.
    Stop {
        reason: StopReason,
        responder: oneshot::Sender<Result<(), ActorError>>,
    },

    /// Reset the crash loop policy.
    ResetCrashLoop {
        responder: oneshot::Sender<Result<(), ActorError>>,
    },

    /// Query the current state.
    GetState {
        responder: oneshot::Sender<RuntimeState>,
    },
}

/// Events from the I/O layer or process watchers.
#[derive(Debug)]
pub enum ActorEvent {
    /// A process tree handle was obtained (spawn succeeded).
    TreeHandleObtained {
        generation: Generation,
        pid: Pid,
    },

    /// The spawn failed before getting a tree handle.
    SpawnFailed {
        generation: Generation,
        error: String,
    },

    /// `$/initialize` completed successfully.
    InitializeComplete {
        generation: Generation,
    },

    /// `$/activate` completed successfully.
    ActivateComplete {
        generation: Generation,
    },

    /// A handshake phase failed.
    HandshakeFailed {
        generation: Generation,
        phase: HandshakePhase,
        error: String,
    },

    /// The direct Bun process exited.
    DirectProcessExited {
        generation: Generation,
        exit: ProcessExit,
    },

    /// Clean EOF on stdout (frame boundary).
    StdoutEof {
        generation: Generation,
    },

    /// stdout read failure.
    StdoutReadFailed {
        generation: Generation,
        message: String,
    },

    /// Writer failure.
    WriterFailed {
        generation: Generation,
        stage: WriterFailureStage,
        message: String,
    },

    /// The Job Object tree is confirmed empty.
    TreeBecameEmpty {
        generation: Generation,
    },

    /// A protocol violation was detected.
    ProtocolViolation {
        generation: Generation,
        message: String,
    },
}

/// Which handshake phase failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakePhase {
    Initialize,
    Activate,
}

/// Errors returned by actor commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorError {
    /// Plugin is in CrashLoop — must reset before starting.
    CrashLoop,
    /// Plugin is not Running — cannot stop.
    NotRunning,
    /// Generation mismatch (stale event).
    WrongGeneration { expected: Generation, actual: Generation },
    /// Operation timed out.
    Timeout,
    /// Spawn failed.
    SpawnFailed { reason: String },
    /// Internal error.
    Internal { message: String },
}

impl std::fmt::Display for ActorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CrashLoop => write!(f, "plugin is in crash loop"),
            Self::NotRunning => write!(f, "plugin is not running"),
            Self::WrongGeneration { expected, actual } => {
                write!(f, "generation mismatch: expected {expected}, got {actual}")
            }
            Self::Timeout => write!(f, "operation timed out"),
            Self::SpawnFailed { reason } => write!(f, "spawn failed: {reason}"),
            Self::Internal { message } => write!(f, "internal error: {message}"),
        }
    }
}

/// Configuration for the runtime actor.
#[derive(Debug, Clone)]
pub struct ActorConfig {
    /// Maximum capacity of the mailbox channel.
    pub mailbox_capacity: usize,
    /// Crash window: number of crashes in this many seconds triggers CrashLoop.
    pub crash_window_crashes: u32,
    pub crash_window_secs: u64,
}

impl Default for ActorConfig {
    fn default() -> Self {
        Self {
            mailbox_capacity: 256,
            crash_window_crashes: 3,
            crash_window_secs: 300, // 5 minutes
        }
    }
}

/// The runtime actor for a single plugin.
///
/// Owns the state machine, processes commands and events sequentially,
/// and enforces generation isolation.
pub struct RuntimeActor {
    state: RuntimeState,
    command_rx: mpsc::Receiver<ActorCommand>,
    event_rx: mpsc::Receiver<ActorEvent>,
    config: ActorConfig,
    /// Recent crash timestamps (Unix milliseconds) for sliding window.
    recent_crashes: Vec<u64>,
}

impl RuntimeActor {
    /// Create the actor and return the command/event senders for external use.
    pub fn new(config: ActorConfig) -> (Self, mpsc::Sender<ActorCommand>, mpsc::Sender<ActorEvent>) {
        let (cmd_tx, cmd_rx) = mpsc::channel(config.mailbox_capacity);
        let (evt_tx, evt_rx) = mpsc::channel(config.mailbox_capacity);
        let actor = Self {
            state: RuntimeState::Stopped,
            command_rx: cmd_rx,
            event_rx: evt_rx,
            config,
            recent_crashes: Vec::new(),
        };
        (actor, cmd_tx, evt_tx)
    }

    /// Run the actor loop. Blocks until the command channel is closed and
    /// all pending events are drained.
    pub async fn run(&mut self, mut now_ms_fn: impl FnMut() -> u64) {
        loop {
            tokio::select! {
                // Commands take priority
                biased;

                Some(cmd) = self.command_rx.recv() => {
                    self.handle_command(cmd).await;
                }
                Some(event) = self.event_rx.recv() => {
                    self.handle_event(event, &mut now_ms_fn);
                }
                else => break,
            }
        }
    }

    async fn handle_command(&mut self, cmd: ActorCommand) {
        match cmd {
            ActorCommand::Start { responder } => {
                let result = self.handle_start();
                let _ = responder.send(result);
            }
            ActorCommand::Stop { reason, responder } => {
                let result = self.handle_stop(reason);
                let _ = responder.send(result);
            }
            ActorCommand::ResetCrashLoop { responder } => {
                let result = self.handle_reset_crash_loop();
                let _ = responder.send(result);
            }
            ActorCommand::GetState { responder } => {
                let _ = responder.send(self.state.clone());
            }
        }
    }

    fn handle_event(&mut self, event: ActorEvent, now_ms_fn: &mut impl FnMut() -> u64) {
        match event {
            ActorEvent::TreeHandleObtained { generation, pid } => {
                if self.state.generation() != Some(generation) {
                    return; // stale event
                }
                let new_state = RuntimeState::Initializing { generation, pid };
                if validate_transition(&self.state, &new_state).is_ok() {
                    self.state = new_state;
                }
            }
            ActorEvent::SpawnFailed { generation, error: _ } => {
                if self.state.generation() != Some(generation) {
                    return;
                }
                self.state = RuntimeState::Stopped;
            }
            ActorEvent::InitializeComplete { generation } => {
                if self.state.generation() != Some(generation) {
                    return;
                }
                if let RuntimeState::Initializing { pid, .. } = &self.state {
                    let new_state = RuntimeState::Activating {
                        generation,
                        pid: *pid,
                    };
                    if validate_transition(&self.state, &new_state).is_ok() {
                        self.state = new_state;
                    }
                }
            }
            ActorEvent::ActivateComplete { generation } => {
                if self.state.generation() != Some(generation) {
                    return;
                }
                if let RuntimeState::Activating { pid, .. } = &self.state {
                    let new_state = RuntimeState::Running {
                        generation,
                        pid: *pid,
                    };
                    if validate_transition(&self.state, &new_state).is_ok() {
                        self.state = new_state;
                    }
                }
            }
            ActorEvent::HandshakeFailed { generation, .. } => {
                if self.state.generation() != Some(generation) {
                    return;
                }
                self.state = RuntimeState::Stopped;
            }
            ActorEvent::DirectProcessExited { generation, exit } => {
                self.handle_unexpected_exit(generation, exit, now_ms_fn);
            }
            ActorEvent::StdoutEof { generation } => {
                self.handle_drain(generation, DrainTrigger::StdoutBoundaryEof, now_ms_fn);
            }
            ActorEvent::StdoutReadFailed { generation, message } => {
                self.handle_drain(
                    generation,
                    DrainTrigger::StdoutReadFailure { message },
                    now_ms_fn,
                );
            }
            ActorEvent::WriterFailed {
                generation,
                stage,
                message,
            } => {
                self.handle_drain(
                    generation,
                    DrainTrigger::WriterFailure { stage, message },
                    now_ms_fn,
                );
            }
            ActorEvent::TreeBecameEmpty { generation } => {
                match &mut self.state {
                    RuntimeState::Draining {
                        progress,
                        generation: gen_id,
                        ..
                    } if *gen_id == generation => {
                        progress.tree = super::state::TreeDrain::Empty;
                        self.check_drain_complete();
                    }
                    _ => {}
                }
            }
            ActorEvent::ProtocolViolation {
                generation,
                message,
            } => {
                self.handle_drain(
                    generation,
                    DrainTrigger::ProtocolFailure { message },
                    now_ms_fn,
                );
            }
        }
    }

    fn handle_start(&mut self) -> Result<(), ActorError> {
        match &self.state {
            RuntimeState::CrashLoop { .. } => Err(ActorError::CrashLoop),
            RuntimeState::Stopped | RuntimeState::Crashed { .. } => {
                let next_gen = self.state.generation().unwrap_or(0) + 1;
                let token = SpawnToken(next_gen);
                self.state = RuntimeState::Starting {
                    generation: next_gen,
                    spawn_token: token,
                };
                Ok(())
            }
            RuntimeState::Starting { .. }
            | RuntimeState::Initializing { .. }
            | RuntimeState::Activating { .. }
            | RuntimeState::Running { .. } => {
                // Already starting or running — join the existing generation
                Ok(())
            }
            _ => Err(ActorError::Internal {
                message: format!("cannot start from state {}", self.state.label()),
            }),
        }
    }

    fn handle_stop(&mut self, reason: StopReason) -> Result<(), ActorError> {
        match &self.state {
            RuntimeState::Running { generation, pid }
            | RuntimeState::Stopping { generation, pid, .. } => {
                let new_state = RuntimeState::Stopping {
                    generation: *generation,
                    pid: *pid,
                    reason,
                };
                if validate_transition(&self.state, &new_state).is_ok() {
                    self.state = new_state;
                }
                Ok(())
            }
            RuntimeState::Starting {
                generation,
                spawn_token,
            } => {
                let new_state = RuntimeState::CancellingStart {
                    generation: *generation,
                    spawn_token: *spawn_token,
                    reason,
                };
                if validate_transition(&self.state, &new_state).is_ok() {
                    self.state = new_state;
                }
                Ok(())
            }
            _ => Err(ActorError::NotRunning),
        }
    }

    fn handle_reset_crash_loop(&mut self) -> Result<(), ActorError> {
        if matches!(self.state, RuntimeState::CrashLoop { .. }) {
            self.state = RuntimeState::Stopped;
            self.recent_crashes.clear();
            Ok(())
        } else {
            Err(ActorError::Internal {
                message: "not in crash loop".to_string(),
            })
        }
    }

    /// Handle an unexpected process exit.
    fn handle_unexpected_exit(
        &mut self,
        generation: Generation,
        exit: ProcessExit,
        now_ms_fn: &mut impl FnMut() -> u64,
    ) {
        if self.state.generation() != Some(generation) {
            return;
        }

        if matches!(self.state, RuntimeState::Running { .. }) {
            // Running → crash
            let now = now_ms_fn();
            self.record_crash(now);

            if self.is_crash_loop(now) {
                self.state = RuntimeState::CrashLoop {
                    recent_crashes: self.recent_crashes.len() as u32,
                };
            } else {
                self.state = RuntimeState::Crashed {
                    generation,
                    exit,
                };
            }
        } else {
            // Not in Running — enter Draining
            self.state = RuntimeState::Draining {
                generation,
                primary_trigger: DrainTrigger::DirectProcessExit,
                progress: DrainProgress::default(),
            };
        }
    }

    /// Transition to Draining on EOF/failure.
    fn handle_drain(
        &mut self,
        generation: Generation,
        trigger: DrainTrigger,
        _now_ms_fn: &mut impl FnMut() -> u64,
    ) {
        if self.state.generation() != Some(generation) {
            return;
        }
        self.state = RuntimeState::Draining {
            generation,
            primary_trigger: trigger,
            progress: DrainProgress::default(),
        };
    }

    /// Check if all drain conditions are satisfied → Stopped.
    fn check_drain_complete(&mut self) {
        if let RuntimeState::Draining { progress, .. } = &self.state {
            if matches!(progress.direct_process, super::state::DirectProcessDrain::Reaped { .. })
                && matches!(progress.stdout, super::state::PipeDrain::BoundaryEof)
                && matches!(progress.tree, super::state::TreeDrain::Empty)
            {
                self.state = RuntimeState::Stopped;
            }
        }
    }

    fn record_crash(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.config.crash_window_secs * 1000);
        self.recent_crashes.retain(|t| *t > cutoff);
        self.recent_crashes.push(now_ms);
    }

    fn is_crash_loop(&self, _now_ms: u64) -> bool {
        self.recent_crashes.len() >= self.config.crash_window_crashes as usize
    }

    /// Returns a clone of the current state (for testing).
    pub fn state(&self) -> &RuntimeState {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn make_actor() -> (RuntimeActor, mpsc::Sender<ActorCommand>, mpsc::Sender<ActorEvent>) {
        RuntimeActor::new(ActorConfig::default())
    }

    fn fixed_now() -> u64 {
        1_000_000_000
    }

    #[tokio::test]
    async fn start_from_stopped() {
        let (mut actor, _cmd_tx, _evt_tx) = make_actor();
        // Test the handler directly without running the actor loop
        let result = actor.handle_start();
        assert!(result.is_ok());
        assert!(matches!(actor.state, RuntimeState::Starting { .. }));
    }

    #[tokio::test]
    async fn start_from_crash_loop_fails() {
        let (mut actor, cmd_tx, _evt_tx) = make_actor();
        actor.state = RuntimeState::CrashLoop { recent_crashes: 3 };
        let result = actor.handle_start();
        assert_eq!(result, Err(ActorError::CrashLoop));
    }

    #[tokio::test]
    async fn stop_running() {
        let (mut actor, _cmd_tx, _evt_tx) = make_actor();
        actor.state = RuntimeState::Running {
            generation: 1,
            pid: 100,
        };
        let result = actor.handle_stop(StopReason::ManualStop);
        assert!(result.is_ok());
        assert!(matches!(actor.state, RuntimeState::Stopping { .. }));
    }

    #[tokio::test]
    async fn stop_from_stopped_fails() {
        let (mut actor, _cmd_tx, _evt_tx) = make_actor();
        let result = actor.handle_stop(StopReason::ManualStop);
        assert_eq!(result, Err(ActorError::NotRunning));
    }

    #[test]
    fn reset_crash_loop_works() {
        let (mut actor, _cmd_tx, _evt_tx) = make_actor();
        actor.state = RuntimeState::CrashLoop { recent_crashes: 3 };
        assert!(actor.handle_reset_crash_loop().is_ok());
        assert!(matches!(actor.state, RuntimeState::Stopped));
    }

    #[test]
    fn crash_recording_window() {
        let (mut actor, _cmd_tx, _evt_tx) = make_actor();
        // Record 3 crashes at the same time
        actor.record_crash(1_000_000_000);
        actor.record_crash(1_000_000_000);
        actor.record_crash(1_000_000_000);
        assert!(actor.is_crash_loop(1_000_000_000));
    }

    #[test]
    fn stale_event_ignored() {
        let (mut actor, _cmd_tx, _evt_tx) = make_actor();
        actor.state = RuntimeState::Running {
            generation: 5,
            pid: 100,
        };
        // Event from old generation should be silently ignored
        let event = ActorEvent::DirectProcessExited {
            generation: 3,
            exit: ProcessExit {
                exit_code: Some(0),
                exit_signal: None,
            },
        };
        let mut now = || fixed_now();
        actor.handle_event(event, &mut now);
        // State should still be Running
        assert!(matches!(actor.state, RuntimeState::Running { .. }));
    }

    #[test]
    fn running_crash_triggers_crash_record() {
        let (mut actor, _cmd_tx, _evt_tx) = make_actor();
        actor.state = RuntimeState::Running {
            generation: 1,
            pid: 100,
        };
        let event = ActorEvent::DirectProcessExited {
            generation: 1,
            exit: ProcessExit {
                exit_code: Some(1),
                exit_signal: None,
            },
        };
        let mut now = || fixed_now();
        actor.handle_event(event, &mut now);
        assert!(matches!(actor.state, RuntimeState::Crashed { .. }));
    }
}
