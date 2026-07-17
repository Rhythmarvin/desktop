//! Plugin runtime — actor-based lifecycle management for Agent plugin processes.

pub mod assets;
mod generation;
pub mod handshake;
mod hub;
mod invocation;
pub mod outcome;
pub mod pending;
mod session_actor;
mod startup;
pub(crate) mod state;
pub mod supervisor;
#[cfg(test)]
mod tests;
mod transport;

pub use assets::*;
pub use generation::*;
pub use handshake::*;
pub use hub::*;
pub use invocation::*;
pub use outcome::*;
pub use pending::*;
pub(crate) use session_actor::*;
pub(crate) use startup::*;
pub use state::*;
pub use supervisor::*;
pub use transport::*;
