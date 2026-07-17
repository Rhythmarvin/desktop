//! Pending request table — tracks in-flight Host→Plugin RPC calls.
//!
//! Each request passes through states: Queued → WriteStarted → Written → Cancelling.
//! A monotonic `actor_sequence` orders all events within a generation.
//! Termination intents and fatal settlement causes are write-once to guarantee
//! deterministic outcomes even under concurrent cancel/deadline/crash races.

use super::state::Generation;
use std::collections::HashMap;

// ── Actor sequence ──────────────────────────────────────────────

/// Monotonic sequence number for ordering events within a generation.
pub type ActorSequence = u64;

// ── Write state ──────────────────────────────────────────────────

/// How far a request's frame got into the write pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteState {
    /// Not yet picked up by the writer.
    Queued,
    /// Writer has started; partial bytes may have been written.
    WriteStarted { bytes_written: usize },
    /// The complete `5 + N` bytes were `write_all`-ed successfully.
    Written,
    /// Cancellation is in progress after a Written frame.
    Cancelling,
}

/// Result of a single frame write attempt.
#[derive(Debug, Clone)]
pub enum WriteAck {
    /// The complete frame was written.
    FrameWritten,
    /// The write failed (0 = NotWritten, >0 = PossiblyWritten).
    WriteFailed { bytes_written: usize },
}

// ── Termination intents ─────────────────────────────────────────

/// Why this request is being terminated (first effective intent wins).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminationIntent {
    /// Caller explicitly cancelled.
    ExplicitCancel,
    /// Host stop (disable/uninstall/shutdown).
    HostStop,
    /// Backpressure — consumer channel full.
    Backpressure,
    /// Invocation deadline expired.
    HardDeadline,
}

impl TerminationIntent {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ExplicitCancel => "ExplicitCancel",
            Self::HostStop => "HostStop",
            Self::Backpressure => "Backpressure",
            Self::HardDeadline => "HardDeadline",
        }
    }
}

// ── Fatal settlement ────────────────────────────────────────────

/// Why the connection entered fatal drain (write-once per generation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FatalSettlementCause {
    /// Connection was lost at a specific stage.
    ConnectionLost { stage: ConnectionStage },
    /// The process exited with the given code.
    ProcessExited { exit_code: Option<i32> },
}

/// Where in the connection lifecycle the failure occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStage {
    ResponseRead,
    RequestWrite,
    TransportCancelWrite,
    SessionDrain,
}

// ── Request entry ───────────────────────────────────────────────

/// One pending request tracked by the runtime actor.
#[derive(Debug, Clone)]
pub struct PendingEntry {
    /// The JSON-RPC request id (e.g., "h:42").
    pub request_id: String,
    /// The method being invoked.
    pub method: String,
    /// Generation that owns this request.
    pub generation: Generation,
    /// Actor sequence when this entry was created.
    pub created_at: ActorSequence,
    /// Whether the method is idempotent.
    pub idempotent: bool,
    /// Whether this is a safety-control method (cancelConversation).
    pub safety: bool,
    /// Current write state.
    pub write_state: WriteState,
    /// First effective termination intent (write-once).
    pub intent: Option<TerminationIntent>,
    /// Sequence of the intent (for tiebreaking).
    pub intent_sequence: Option<ActorSequence>,
    /// Fatal cause if the session entered drain (write-once).
    pub fatal_cause: Option<FatalSettlementCause>,
}

impl PendingEntry {
    pub fn new(
        request_id: String,
        method: String,
        generation: Generation,
        sequence: ActorSequence,
        idempotent: bool,
        safety: bool,
    ) -> Self {
        Self {
            request_id,
            method,
            generation,
            created_at: sequence,
            idempotent,
            safety,
            write_state: WriteState::Queued,
            intent: None,
            intent_sequence: None,
            fatal_cause: None,
        }
    }

    /// Lock a termination intent. Returns false if a different intent already exists.
    pub fn set_intent(&mut self, intent: TerminationIntent, seq: ActorSequence) -> bool {
        if let Some(existing) = &self.intent {
            // Same intent type → merge (keep earlier sequence)
            if *existing == intent {
                return true;
            }
            // Different intent → first wins
            return false;
        }
        self.intent = Some(intent);
        self.intent_sequence = Some(seq);
        true
    }

    /// Lock a fatal settlement cause. Returns false if already set.
    pub fn set_fatal_cause(&mut self, cause: FatalSettlementCause) -> bool {
        if self.fatal_cause.is_some() {
            return false;
        }
        self.fatal_cause = Some(cause);
        true
    }
}

// ── Pending table ───────────────────────────────────────────────

/// The actor-owned pending request table.
pub struct PendingTable {
    entries: HashMap<String, PendingEntry>,
    max_ordinary: usize,
    next_sequence: ActorSequence,
}

impl PendingTable {
    /// Create a new pending table with a max ordinary capacity.
    pub fn new(max_ordinary: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_ordinary,
            next_sequence: 0,
        }
    }

    /// Allocate the next actor sequence number.
    pub fn next_sequence(&mut self) -> ActorSequence {
        self.next_sequence += 1;
        self.next_sequence
    }

    /// Current sequence counter.
    pub fn current_sequence(&self) -> ActorSequence {
        self.next_sequence
    }

    /// Insert a new pending entry.
    pub fn insert(&mut self, entry: PendingEntry) -> Result<(), &'static str> {
        if self.entries.contains_key(&entry.request_id) {
            return Err("duplicate request id");
        }
        let ordinary_count = self.entries.values().filter(|e| !e.safety).count();
        if !entry.safety && ordinary_count >= self.max_ordinary {
            return Err("ordinary pending table full");
        }
        self.entries.insert(entry.request_id.clone(), entry);
        Ok(())
    }

    /// Get a mutable reference to an entry.
    pub fn get_mut(&mut self, request_id: &str) -> Option<&mut PendingEntry> {
        self.entries.get_mut(request_id)
    }

    /// Get a reference to an entry.
    pub fn get(&self, request_id: &str) -> Option<&PendingEntry> {
        self.entries.get(request_id)
    }

    /// Remove an entry (after terminal response).
    pub fn remove(&mut self, request_id: &str) -> Option<PendingEntry> {
        self.entries.remove(request_id)
    }

    /// Count of active entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Count of ordinary (non-safety) entries.
    pub fn ordinary_count(&self) -> usize {
        self.entries.values().filter(|e| !e.safety).count()
    }

    /// Count of safety entries.
    pub fn safety_count(&self) -> usize {
        self.entries.values().filter(|e| e.safety).count()
    }

    /// Check if there is capacity for a new ordinary request.
    pub fn has_ordinary_capacity(&self) -> bool {
        self.ordinary_count() < self.max_ordinary
    }

    /// Set the fatal cause on all entries that don't have one yet.
    pub fn set_fatal_cause_all(&mut self, cause: FatalSettlementCause) {
        for entry in self.entries.values_mut() {
            entry.set_fatal_cause(cause.clone());
        }
    }

    /// Drain all entries (return them for settlement).
    pub fn drain_all(&mut self) -> Vec<PendingEntry> {
        self.entries.drain().map(|(_, v)| v).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn make_entry(id: &str) -> PendingEntry {
        PendingEntry::new(id.to_string(), "agent.test".to_string(), 1, 1, true, false)
    }

    #[test]
    fn insert_and_retrieve() {
        let mut table = PendingTable::new(128);
        let entry = make_entry("h:1");
        assert!(table.insert(entry).is_ok());
        assert_eq!(table.len(), 1);
        assert!(table.get("h:1").is_some());
    }

    #[test]
    fn duplicate_request_id_rejected() {
        let mut table = PendingTable::new(128);
        table.insert(make_entry("h:1")).unwrap();
        assert!(table.insert(make_entry("h:1")).is_err());
    }

    #[test]
    fn ordinary_capacity_enforced() {
        let mut table = PendingTable::new(2);
        table.insert(make_entry("h:1")).unwrap();
        table.insert(make_entry("h:2")).unwrap();
        assert!(table.insert(make_entry("h:3")).is_err());
        assert!(!table.has_ordinary_capacity());
    }

    #[test]
    fn safety_entries_dont_count_against_ordinary_capacity() {
        let mut table = PendingTable::new(1);
        // Fill the only ordinary slot
        table.insert(make_entry("h:1")).unwrap();
        // Safety entry should still work
        let mut safety = make_entry("h:safe");
        safety.safety = true;
        assert!(table.insert(safety).is_ok());
    }

    #[test]
    fn termination_intent_first_wins() {
        let mut entry = make_entry("h:1");
        assert!(entry.set_intent(TerminationIntent::ExplicitCancel, 1));
        // Different intent should be rejected
        assert!(!entry.set_intent(TerminationIntent::HostStop, 2));
        assert_eq!(entry.intent, Some(TerminationIntent::ExplicitCancel));
    }

    #[test]
    fn termination_intent_same_type_merges() {
        let mut entry = make_entry("h:1");
        assert!(entry.set_intent(TerminationIntent::ExplicitCancel, 1));
        // Same type, later sequence → still accepted (merge)
        assert!(entry.set_intent(TerminationIntent::ExplicitCancel, 2));
        // Sequence stays at the earlier one
        assert_eq!(entry.intent_sequence, Some(1));
    }

    #[test]
    fn fatal_cause_write_once() {
        let mut entry = make_entry("h:1");
        assert!(entry.set_fatal_cause(FatalSettlementCause::ConnectionLost {
            stage: ConnectionStage::ResponseRead,
        }));
        // Second write should be rejected
        assert!(!entry.set_fatal_cause(FatalSettlementCause::ProcessExited {
            exit_code: Some(1),
        }));
        assert!(matches!(
            entry.fatal_cause,
            Some(FatalSettlementCause::ConnectionLost { .. })
        ));
    }

    #[test]
    fn set_fatal_cause_all() {
        let mut table = PendingTable::new(128);
        table.insert(make_entry("h:1")).unwrap();
        table.insert(make_entry("h:2")).unwrap();
        table.set_fatal_cause_all(FatalSettlementCause::ProcessExited {
            exit_code: Some(0),
        });
        assert!(matches!(
            table.get("h:1").unwrap().fatal_cause,
            Some(FatalSettlementCause::ProcessExited { .. })
        ));
        assert!(matches!(
            table.get("h:2").unwrap().fatal_cause,
            Some(FatalSettlementCause::ProcessExited { .. })
        ));
    }

    #[test]
    fn sequence_monotonic() {
        let mut table = PendingTable::new(128);
        let s1 = table.next_sequence();
        let s2 = table.next_sequence();
        assert!(s2 > s1);
    }

    #[test]
    fn drain_clears_table() {
        let mut table = PendingTable::new(128);
        table.insert(make_entry("h:1")).unwrap();
        table.insert(make_entry("h:2")).unwrap();
        let drained = table.drain_all();
        assert_eq!(drained.len(), 2);
        assert_eq!(table.len(), 0);
    }
}
