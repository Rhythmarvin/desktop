use ora_application::NorthboundBus;
use ora_contracts::Northbound;
use tokio::sync::broadcast;

const BROADCAST_CAPACITY: usize = 256;

/// Broadcasts backend `Northbound` events to all connected SSE clients.
///
/// Slow consumers observe a lag signal after the channel discards buffered events.
#[derive(Clone, Debug)]
pub struct BroadcastNorthboundBus {
    sender: broadcast::Sender<Northbound>,
}

impl BroadcastNorthboundBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self { sender }
    }

    /// Returns a new receiver for one SSE client connection.
    pub fn subscribe(&self) -> broadcast::Receiver<Northbound> {
        self.sender.subscribe()
    }
}

impl Default for BroadcastNorthboundBus {
    fn default() -> Self {
        Self::new()
    }
}

impl NorthboundBus for BroadcastNorthboundBus {
    fn emit(&self, event: Northbound) {
        // Ignore send errors (no receivers). This is best-effort delivery.
        let _ = self.sender.send(event);
    }
}
