use crate::distributed::models::SystemEventRecord;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<SystemEventRecord>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn publish(&self, event: SystemEventRecord) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SystemEventRecord> {
        self.sender.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(10_000)
    }
}
