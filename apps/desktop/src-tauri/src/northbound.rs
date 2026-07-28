use ora_application::NorthboundBus;
use ora_contracts::Northbound;
use tauri::AppHandle;

/// Delivers northbound events to the Desktop frontend via Tauri IPC.
///
/// `AppHandle::emit` is synchronous and non-blocking, matching the
/// `NorthboundBus` fire-and-forget contract.
pub struct TauriNorthboundBus {
    handle: AppHandle,
}

impl TauriNorthboundBus {
    pub fn new(handle: AppHandle) -> Self {
        Self { handle }
    }
}

impl NorthboundBus for TauriNorthboundBus {
    fn emit(&self, event: Northbound) {
        // Tauri 2.x emit is fire-and-forget; serialization errors are logged
        // internally by Tauri. We ignore the Result to match the best-effort
        // contract — if the frontend is not listening the event is irrelevant.
        let _ = self.handle.emit("northbound", &event);
    }
}
