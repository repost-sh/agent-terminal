use std::sync::Arc;

use tokio::sync::watch;

#[derive(Clone)]
pub struct Changes(Arc<watch::Sender<u64>>);

impl Default for Changes {
    fn default() -> Self {
        Self(Arc::new(watch::Sender::new(0)))
    }
}

impl Changes {
    pub fn notify(&self) {
        self.0.send_modify(|count| *count = count.wrapping_add(1));
    }

    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.0.subscribe()
    }
}
