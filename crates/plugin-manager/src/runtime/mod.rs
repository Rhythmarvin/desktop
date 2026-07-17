//! Plugin runtime — actor-based lifecycle management for Agent plugin processes.
//!
//! ## Architecture
//!
//! Each plugin gets one [`RuntimeActor`] that owns:
//! - An 11-state state machine ([`RuntimeState`])
//! - A bounded command mailbox ([`ActorCommand`])
//! - A bounded event mailbox ([`ActorEvent`])
//!
//! The [`PluginRuntimeSupervisor`] manages the per-plugin actor registry and
//! routes start/stop/invoke commands to the correct actor.

pub mod actor;
pub mod assets;
pub mod facade;
mod generation;
pub mod handshake;
mod hub;
mod invocation;
pub mod outcome;
pub mod pending;
mod session_actor;
mod startup;
pub mod state;
pub mod supervisor;
#[cfg(test)]
mod tests;
mod transport;

pub use actor::{ActorCommand, ActorConfig, ActorError, ActorEvent, HandshakePhase, RuntimeActor};
pub use assets::{AssetIdentity, AssetReceipt, RuntimeAssetStore};
pub use facade::{AgentPluginRuntime, InvokeResult, RuntimeError};
pub use generation::*;
pub use handshake::{
    build_activate_request, build_cancel_request, build_deactivate_request,
    build_exit_notification, build_initialize_request, build_initialize_response,
    parse_activate_response, parse_initialize_response, ActivateOutcome, InitializeLimits,
    InitializeOutcome, ProviderDescriptor,
};
pub use hub::*;
pub use invocation::*;
pub use outcome::{
    settle_outcome, InvocationOutcome, StreamError, StreamEvent, StreamValidator,
    UnknownOutcomeCause,
};
pub use pending::{
    ActorSequence, ConnectionStage, FatalSettlementCause, PendingEntry, PendingTable,
    TerminationIntent, WriteAck, WriteState,
};
pub(crate) use session_actor::*;
pub(crate) use startup::*;
pub use state::{
    DrainProgress, DrainTrigger, DirectProcessDrain, PipeDrain, ProcessExit, RuntimeState,
    SpawnToken, StopReason, TreeDrain, WriterFailureStage,
};
pub use supervisor::{ActorHandle, PluginRuntimeSupervisor};
pub use transport::*;
