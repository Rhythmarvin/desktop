//! Invocation outcomes — stream validation, backpressure, cancel/deadline settlement,
//! and the complete outcome matrix for all combinations of write state, idempotency,
//! and fatal cause.

use super::pending::{FatalSettlementCause, TerminationIntent, WriteState};
use super::state::Generation;

// ── Invocation outcome (the caller-facing result) ────────────────

/// The final outcome of an invocation, surfaced to the API caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvocationOutcome {
    /// Successful completion with a typed result.
    Success,
    /// Agent business error (code -32000).
    BusinessError {
        kind: String,
        message: String,
        retryable: bool,
    },
    /// Transport failure (connection lost before write or before response).
    TransportFailed {
        stage: String,
    },
    /// The plugin process exited before the response.
    PluginExited {
        exit_code: Option<i32>,
    },
    /// The request was cancelled before reaching the plugin.
    Cancelled,
    /// The request timed out before reaching the plugin.
    RequestTimedOut,
    /// Backpressure exceeded — consumer channel full.
    BackpressureExceeded,
    /// Non-idempotent request: written but no terminal (UnknownOutcome).
    UnknownOutcome {
        cause: UnknownOutcomeCause,
    },
    /// Safety cancelConversation: cancellation result unconfirmed.
    CancellationUnconfirmed,
}

/// Why a non-idempotent request ended in UnknownOutcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownOutcomeCause {
    CancellationUnconfirmed,
    DeadlineExceeded,
    ConnectionLost,
    ProcessExited,
}

impl std::fmt::Display for UnknownOutcomeCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CancellationUnconfirmed => write!(f, "CancellationUnconfirmed"),
            Self::DeadlineExceeded => write!(f, "DeadlineExceeded"),
            Self::ConnectionLost => write!(f, "ConnectionLost"),
            Self::ProcessExited => write!(f, "ProcessExited"),
        }
    }
}

// ── Stream event ─────────────────────────────────────────────────

/// A single stream event from a streaming method.
#[derive(Debug, Clone)]
pub struct StreamEvent {
    /// The request id this event belongs to.
    pub request_id: String,
    /// Monotonically increasing sequence number (starting at 1).
    pub seq: u64,
    /// The event value (typed stream-event discriminant).
    pub value: serde_json::Value,
}

// ── Stream validation ────────────────────────────────────────────

/// Validate a `$/stream` notification for seq ordering.
#[derive(Debug, Clone)]
pub struct StreamValidator {
    /// The last seen seq for this request.
    last_seq: u64,
    /// Has the terminal response already been sent?
    terminal_sent: bool,
}

impl StreamValidator {
    pub fn new() -> Self {
        Self {
            last_seq: 0,
            terminal_sent: false,
        }
    }

    /// Validate the next seq. Returns Ok if valid, Err if gap/duplicate/after-terminal.
    pub fn validate_seq(&mut self, seq: u64) -> Result<(), StreamError> {
        if self.terminal_sent {
            return Err(StreamError::StreamAfterTerminal);
        }
        if seq == 0 {
            return Err(StreamError::SeqMustStartAtOne);
        }
        let expected = self.last_seq + 1;
        if seq < expected {
            return Err(StreamError::DuplicateOrOutOfOrder {
                expected,
                actual: seq,
            });
        }
        if seq > expected {
            return Err(StreamError::Gap {
                expected,
                actual: seq,
            });
        }
        self.last_seq = seq;
        Ok(())
    }

    /// Mark the terminal as sent — any further stream events are fatal.
    pub fn mark_terminal(&mut self) {
        self.terminal_sent = true;
    }

    /// Current seq (used for `causal_after_seq`).
    pub fn current_seq(&self) -> u64 {
        self.last_seq
    }
}

impl Default for StreamValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Stream validation errors — all are fatal protocol violations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamError {
    /// seq did not start at 1.
    SeqMustStartAtOne,
    /// A seq was skipped (gap).
    Gap { expected: u64, actual: u64 },
    /// A seq was duplicated or out of order.
    DuplicateOrOutOfOrder { expected: u64, actual: u64 },
    /// A stream event arrived after the terminal response.
    StreamAfterTerminal,
    /// A non-streaming method tried to send stream events.
    MethodNotStreaming,
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SeqMustStartAtOne => write!(f, "stream seq must start at 1"),
            Self::Gap { expected, actual } => {
                write!(f, "stream seq gap: expected {expected}, got {actual}")
            }
            Self::DuplicateOrOutOfOrder { expected, actual } => {
                write!(
                    f,
                    "stream seq duplicate/out-of-order: expected {expected}, got {actual}"
                )
            }
            Self::StreamAfterTerminal => {
                write!(f, "stream event after terminal response")
            }
            Self::MethodNotStreaming => {
                write!(f, "stream events not allowed for non-streaming method")
            }
        }
    }
}

// ── Outcome settlement ───────────────────────────────────────────

/// Determine the invocation outcome from write state, idempotency, and fatal cause.
///
/// This implements the fatal-settlement matrix from design-v3.md §12.6 and
/// plugin_sdk_final_design_cn.md Appendix D.
pub fn settle_outcome(
    write_state: WriteState,
    idempotent: bool,
    intent: Option<TerminationIntent>,
    fatal_cause: Option<&FatalSettlementCause>,
) -> InvocationOutcome {
    // ── Has a termination intent? Use the first-intent matrix ────
    if let Some(intent) = intent {
        return match intent {
            TerminationIntent::ExplicitCancel | TerminationIntent::HostStop => {
                if matches!(write_state, WriteState::Queued) {
                    InvocationOutcome::Cancelled
                } else if idempotent {
                    InvocationOutcome::Cancelled
                } else {
                    InvocationOutcome::UnknownOutcome {
                        cause: UnknownOutcomeCause::CancellationUnconfirmed,
                    }
                }
            }
            TerminationIntent::Backpressure => {
                if matches!(write_state, WriteState::Queued) {
                    InvocationOutcome::BackpressureExceeded
                } else if idempotent {
                    InvocationOutcome::BackpressureExceeded
                } else {
                    InvocationOutcome::UnknownOutcome {
                        cause: UnknownOutcomeCause::CancellationUnconfirmed,
                    }
                }
            }
            TerminationIntent::HardDeadline => {
                if matches!(write_state, WriteState::Queued) {
                    InvocationOutcome::RequestTimedOut
                } else if idempotent {
                    InvocationOutcome::RequestTimedOut
                } else {
                    InvocationOutcome::UnknownOutcome {
                        cause: UnknownOutcomeCause::DeadlineExceeded,
                    }
                }
            }
        };
    }

    // ── No intent — use fatal cause ─────────────────────────────
    if let Some(cause) = fatal_cause {
        return match cause {
            FatalSettlementCause::ConnectionLost { stage } => {
                if matches!(write_state, WriteState::Queued) {
                    InvocationOutcome::TransportFailed {
                        stage: format!("{stage:?}"),
                    }
                } else if idempotent {
                    InvocationOutcome::TransportFailed {
                        stage: format!("{stage:?}"),
                    }
                } else {
                    InvocationOutcome::UnknownOutcome {
                        cause: UnknownOutcomeCause::ConnectionLost,
                    }
                }
            }
            FatalSettlementCause::ProcessExited { exit_code } => {
                if matches!(write_state, WriteState::Queued) {
                    InvocationOutcome::PluginExited {
                        exit_code: *exit_code,
                    }
                } else if idempotent {
                    InvocationOutcome::PluginExited {
                        exit_code: *exit_code,
                    }
                } else {
                    InvocationOutcome::UnknownOutcome {
                        cause: UnknownOutcomeCause::ProcessExited,
                    }
                }
            }
        };
    }

    // ── No intent, no fatal cause — should not happen ───────────
    InvocationOutcome::TransportFailed {
        stage: "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::pending::ConnectionStage;
    use pretty_assertions::assert_eq;

    #[test]
    fn stream_validator_normal_flow() {
        let mut sv = StreamValidator::new();
        assert!(sv.validate_seq(1).is_ok());
        assert!(sv.validate_seq(2).is_ok());
        assert!(sv.validate_seq(3).is_ok());
        assert_eq!(sv.current_seq(), 3);
    }

    #[test]
    fn stream_validator_rejects_gap() {
        let mut sv = StreamValidator::new();
        sv.validate_seq(1).unwrap();
        let err = sv.validate_seq(3).unwrap_err();
        assert!(matches!(err, StreamError::Gap { .. }));
    }

    #[test]
    fn stream_validator_rejects_duplicate() {
        let mut sv = StreamValidator::new();
        sv.validate_seq(1).unwrap();
        let err = sv.validate_seq(1).unwrap_err();
        assert!(matches!(err, StreamError::DuplicateOrOutOfOrder { .. }));
    }

    #[test]
    fn stream_validator_rejects_after_terminal() {
        let mut sv = StreamValidator::new();
        sv.validate_seq(1).unwrap();
        sv.mark_terminal();
        let err = sv.validate_seq(2).unwrap_err();
        assert!(matches!(err, StreamError::StreamAfterTerminal));
    }

    #[test]
    fn stream_validator_rejects_zero() {
        let mut sv = StreamValidator::new();
        let err = sv.validate_seq(0).unwrap_err();
        assert!(matches!(err, StreamError::SeqMustStartAtOne));
    }

    #[test]
    fn settle_queued_cancelled() {
        let outcome = settle_outcome(
            WriteState::Queued,
            true,
            Some(TerminationIntent::ExplicitCancel),
            None,
        );
        assert_eq!(outcome, InvocationOutcome::Cancelled);
    }

    #[test]
    fn settle_written_non_idempotent_no_terminal() {
        let outcome = settle_outcome(
            WriteState::Written,
            false,
            Some(TerminationIntent::HostStop),
            None,
        );
        assert_eq!(
            outcome,
            InvocationOutcome::UnknownOutcome {
                cause: UnknownOutcomeCause::CancellationUnconfirmed,
            }
        );
    }

    #[test]
    fn settle_written_idempotent_connection_lost() {
        let outcome = settle_outcome(
            WriteState::Written,
            true,
            None,
            Some(&FatalSettlementCause::ConnectionLost {
                stage: ConnectionStage::ResponseRead,
            }),
        );
        assert_eq!(
            outcome,
            InvocationOutcome::TransportFailed {
                stage: "ResponseRead".to_string()
            }
        );
    }

    #[test]
    fn settle_queued_process_exited() {
        let outcome = settle_outcome(
            WriteState::Queued,
            true,
            None,
            Some(&FatalSettlementCause::ProcessExited {
                exit_code: Some(1),
            }),
        );
        assert_eq!(
            outcome,
            InvocationOutcome::PluginExited {
                exit_code: Some(1)
            }
        );
    }

    #[test]
    fn settle_written_non_idempotent_process_exited() {
        let outcome = settle_outcome(
            WriteState::Written,
            false,
            None,
            Some(&FatalSettlementCause::ProcessExited {
                exit_code: Some(1),
            }),
        );
        assert_eq!(
            outcome,
            InvocationOutcome::UnknownOutcome {
                cause: UnknownOutcomeCause::ProcessExited,
            }
        );
    }

    #[test]
    fn settle_queued_hard_deadline() {
        let outcome = settle_outcome(
            WriteState::Queued,
            true,
            Some(TerminationIntent::HardDeadline),
            None,
        );
        assert_eq!(outcome, InvocationOutcome::RequestTimedOut);
    }

    #[test]
    fn settle_queued_backpressure() {
        let outcome = settle_outcome(
            WriteState::Queued,
            false,
            Some(TerminationIntent::Backpressure),
            None,
        );
        assert_eq!(outcome, InvocationOutcome::BackpressureExceeded);
    }
}
