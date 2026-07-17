//! Plugin runtime state machine — 11 states with generation isolation.
//!
//! Each generation carries a monotonic `generation: u64` to prevent stale events
//! from old reader/exit watchers from polluting the new process.

/// Monotonic generation identifier. Incremented on each spawn attempt.
pub type Generation = u64;

/// Process identifier for the direct Bun child.
pub type Pid = u32;

/// Reason for stopping a plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// Explicit user stop.
    ManualStop,
    /// Plugin was disabled.
    Disable,
    /// Plugin is being uninstalled.
    Uninstall,
    /// Application shutdown.
    Shutdown,
    /// Launch grant changed.
    GrantChanged,
}

impl std::fmt::Display for StopReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ManualStop => write!(f, "manualStop"),
            Self::Disable => write!(f, "disable"),
            Self::Uninstall => write!(f, "uninstall"),
            Self::Shutdown => write!(f, "shutdown"),
            Self::GrantChanged => write!(f, "grantChanged"),
        }
    }
}

/// Token proving the caller holds the right to spawn this generation.
/// Created in `Starting` and must match to transition out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpawnToken(pub Generation);

/// Process exit information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessExit {
    pub exit_code: Option<i32>,
    pub exit_signal: Option<i32>,
}

/// Why the generation entered Draining.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrainTrigger {
    /// The direct Bun process exited (before or after EOF).
    DirectProcessExit,
    /// All processes in the Job have exited.
    TreeBecameEmpty,
    /// Clean EOF on stdout (frame boundary).
    StdoutBoundaryEof,
    /// stdout read failure.
    StdoutReadFailure { message: String },
    /// Writer failure at a specific stage.
    WriterFailure { stage: WriterFailureStage, message: String },
    /// Protocol violation detected.
    ProtocolFailure { message: String },
    /// Grace deadline expired — escalate to force terminate.
    StopEscalation,
}

/// Which stage the writer was at when it failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriterFailureStage {
    Request,
    TransportCancel,
    SessionControl,
}

/// Progress through the drain phase.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DrainProgress {
    pub direct_process: DirectProcessDrain,
    pub stdout: PipeDrain,
    pub stderr: PipeDrain,
    pub tree: TreeDrain,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum DirectProcessDrain {
    #[default]
    Awaiting,
    Reaped { exit: ProcessExit },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PipeDrain {
    #[default]
    Open,
    BoundaryEof,
    Failed { message: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TreeDrain {
    #[default]
    Active,
    Empty,
}

/// The 11-state runtime state machine with associated data.
#[derive(Debug, Clone)]
pub enum RuntimeState {
    /// No process running.
    Stopped,

    /// Spawn has been requested; waiting for the process tree handle.
    Starting {
        generation: Generation,
        spawn_token: SpawnToken,
    },

    /// Spawn was requested but a stop arrived before the tree handle.
    CancellingStart {
        generation: Generation,
        spawn_token: SpawnToken,
        reason: StopReason,
    },

    /// Process tree created; waiting for `$/initialize` to complete.
    Initializing {
        generation: Generation,
        pid: Pid,
    },

    /// `$/initialize` done; waiting for `$/activate` to complete.
    Activating {
        generation: Generation,
        pid: Pid,
    },

    /// Plugin is fully active and accepting Agent business requests.
    Running {
        generation: Generation,
        pid: Pid,
    },

    /// Graceful stop in progress — deactivate → exit sequence.
    Stopping {
        generation: Generation,
        pid: Pid,
        reason: StopReason,
    },

    /// Process tree has been terminated; waiting for cleanup.
    CleanupPending {
        generation: Generation,
        reason: StopReason,
    },

    /// Draining pipes, waiting for stdout EOF, stderr drain, reap, tree-empty.
    Draining {
        generation: Generation,
        primary_trigger: DrainTrigger,
        progress: DrainProgress,
    },

    /// The generation exited unexpectedly while Running.
    Crashed {
        generation: Generation,
        exit: ProcessExit,
    },

    /// Crash threshold exceeded — all start/invoke calls fail closed.
    CrashLoop {
        recent_crashes: u32,
    },
}

impl RuntimeState {
    /// Returns the generation number if the state carries one.
    pub fn generation(&self) -> Option<Generation> {
        match self {
            Self::Stopped | Self::CrashLoop { .. } => None,
            Self::Starting { generation, .. }
            | Self::CancellingStart { generation, .. }
            | Self::Initializing { generation, .. }
            | Self::Activating { generation, .. }
            | Self::Running { generation, .. }
            | Self::Stopping { generation, .. }
            | Self::CleanupPending { generation, .. }
            | Self::Draining { generation, .. }
            | Self::Crashed { generation, .. } => Some(*generation),
        }
    }

    /// Returns the PID if the state carries one.
    pub fn pid(&self) -> Option<Pid> {
        match self {
            Self::Initializing { pid, .. }
            | Self::Activating { pid, .. }
            | Self::Running { pid, .. }
            | Self::Stopping { pid, .. } => Some(*pid),
            _ => None,
        }
    }

    /// Whether new Agent business requests are accepted in this state.
    pub fn accepts_requests(&self) -> bool {
        matches!(self, Self::Running { .. })
    }

    /// Whether this is a terminal state (no process, no pending cleanup).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Stopped | Self::CrashLoop { .. })
    }

    /// Human-readable label for logging.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Stopped => "Stopped",
            Self::Starting { .. } => "Starting",
            Self::CancellingStart { .. } => "CancellingStart",
            Self::Initializing { .. } => "Initializing",
            Self::Activating { .. } => "Activating",
            Self::Running { .. } => "Running",
            Self::Stopping { .. } => "Stopping",
            Self::CleanupPending { .. } => "CleanupPending",
            Self::Draining { .. } => "Draining",
            Self::Crashed { .. } => "Crashed",
            Self::CrashLoop { .. } => "CrashLoop",
        }
    }
}

/// Validate that a transition from `from` to `to` is legal.
///
/// Returns `Ok(())` for valid transitions, `Err` with a description otherwise.
pub fn validate_transition(from: &RuntimeState, to: &RuntimeState) -> Result<(), String> {
    use RuntimeState::*;
    match (from, to) {
        // ── Normal start path ─────────────────────────────────
        (Stopped, Starting { .. }) | (CrashLoop { .. }, Starting { .. }) => Ok(()),
        (Starting { .. }, Initializing { .. }) => Ok(()),
        (Initializing { .. }, Activating { .. }) => Ok(()),
        (Activating { .. }, Running { .. }) => Ok(()),

        // ── Start cancelled before tree handle ────────────────
        (Starting { .. }, CancellingStart { .. }) => Ok(()),
        (CancellingStart { .. }, CleanupPending { .. }) => Ok(()),

        // ── Normal stop path ──────────────────────────────────
        (Running { .. }, Stopping { .. })
        | (Initializing { .. }, Stopping { .. })
        | (Activating { .. }, Stopping { .. }) => Ok(()),
        (Stopping { .. }, CleanupPending { .. }) => Ok(()),

        // ── Cleanup → drain → terminal ───────────────────────
        (CleanupPending { .. }, Draining { .. }) => Ok(()),
        (Draining { .. }, Stopped) => Ok(()),
        (Draining { .. }, Crashed { .. }) => Ok(()),
        (Stopping { .. }, Draining { .. }) => Ok(()),

        // ── Crash from Running ────────────────────────────────
        (Running { .. }, Draining { .. }) => Ok(()),
        (Running { .. }, Crashed { .. }) => Ok(()),

        // ── Crash recovery ────────────────────────────────────
        (Crashed { .. }, Starting { .. }) => Ok(()),
        (Crashed { .. }, CrashLoop { .. }) => Ok(()),

        // ── Same-state idempotent ─────────────────────────────
        (a, b) if std::mem::discriminant(a) == std::mem::discriminant(b) => Ok(()),

        _ => Err(format!(
            "illegal state transition: {} -> {}",
            from.label(),
            to.label()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn starting() -> RuntimeState {
        RuntimeState::Starting {
            generation: 1,
            spawn_token: SpawnToken(1),
        }
    }

    fn initializing() -> RuntimeState {
        RuntimeState::Initializing {
            generation: 1,
            pid: 12345,
        }
    }

    fn activating() -> RuntimeState {
        RuntimeState::Activating {
            generation: 1,
            pid: 12345,
        }
    }

    fn running() -> RuntimeState {
        RuntimeState::Running {
            generation: 1,
            pid: 12345,
        }
    }

    #[test]
    fn normal_start_transitions() {
        assert!(validate_transition(&RuntimeState::Stopped, &starting()).is_ok());
        assert!(validate_transition(&starting(), &initializing()).is_ok());
        assert!(validate_transition(&initializing(), &activating()).is_ok());
        assert!(validate_transition(&activating(), &running()).is_ok());
    }

    #[test]
    fn running_to_stopping_is_valid() {
        let stopping = RuntimeState::Stopping {
            generation: 1,
            pid: 12345,
            reason: StopReason::ManualStop,
        };
        assert!(validate_transition(&running(), &stopping).is_ok());
    }

    #[test]
    fn running_to_draining_on_crash() {
        let drain = RuntimeState::Draining {
            generation: 1,
            primary_trigger: DrainTrigger::DirectProcessExit,
            progress: DrainProgress::default(),
        };
        assert!(validate_transition(&running(), &drain).is_ok());
    }

    #[test]
    fn crashed_to_starting_is_valid() {
        let crashed = RuntimeState::Crashed {
            generation: 1,
            exit: ProcessExit {
                exit_code: Some(1),
                exit_signal: None,
            },
        };
        assert!(validate_transition(&crashed, &starting()).is_ok());
    }

    #[test]
    fn crashed_to_crash_loop() {
        let crashed = RuntimeState::Crashed {
            generation: 3,
            exit: ProcessExit {
                exit_code: Some(1),
                exit_signal: None,
            },
        };
        let looped = RuntimeState::CrashLoop { recent_crashes: 3 };
        assert!(validate_transition(&crashed, &looped).is_ok());
    }

    #[test]
    fn stopped_to_running_is_illegal() {
        assert!(validate_transition(&RuntimeState::Stopped, &running()).is_err());
    }

    #[test]
    fn running_to_starting_is_illegal() {
        assert!(validate_transition(&running(), &starting()).is_err());
    }

    #[test]
    fn all_states_have_label() {
        assert_eq!(RuntimeState::Stopped.label(), "Stopped");
        assert_eq!(running().label(), "Running");
        assert_eq!(
            RuntimeState::CrashLoop { recent_crashes: 0 }.label(),
            "CrashLoop"
        );
    }

    #[test]
    fn generation_accessor() {
        assert_eq!(RuntimeState::Stopped.generation(), None);
        assert_eq!(running().generation(), Some(1));
    }

    #[test]
    fn accepts_requests_only_in_running() {
        assert!(!RuntimeState::Stopped.accepts_requests());
        assert!(!starting().accepts_requests());
        assert!(!initializing().accepts_requests());
        assert!(!activating().accepts_requests());
        assert!(running().accepts_requests());
    }
}
