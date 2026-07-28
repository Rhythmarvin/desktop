use ora_contracts::Northbound;

/// Backend-agnostic bus that pushes typed events toward the frontend.
///
/// Implementations deliver events to the correct transport (Tauri IPC for
/// Desktop, SSE broadcast for Web) without `ora-backend` depending on either
/// platform.
pub trait NorthboundBus: Send + Sync {
    /// Pushes one typed northbound event toward connected frontend consumers.
    ///
    /// This is a fire-and-forget operation — delivery failures are silently
    /// ignored because the frontend re-fetches fresh state on its next render
    /// cycle regardless of individual events.
    fn emit(&self, event: Northbound);
}

/// A no-op implementation used in tests and contexts where frontend
/// notifications are irrelevant.
pub struct NoopNorthboundBus;

impl NorthboundBus for NoopNorthboundBus {
    fn emit(&self, _event: Northbound) {}
}
