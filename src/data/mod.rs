use crate::data::snapshot::SystemSnapshot;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

pub mod collector;
pub mod snapshot;

pub struct DataManager {
    pub collector: collector::DataCollector,
    update_interval: Duration,
}

impl DataManager {
    pub fn new(update_interval_ms: u64) -> Self {
        Self {
            collector: collector::DataCollector::new(),
            update_interval: Duration::from_millis(update_interval_ms),
        }
    }

    pub async fn start_polling(&mut self, sender: mpsc::UnboundedSender<SystemSnapshot>) {
        let mut interval = interval(self.update_interval);

        loop {
            interval.tick().await;
            let snapshot = self.collector.collect();

            if sender.send(snapshot).is_err() {
                // Receiver dropped, exit the loop
                break;
            }

            // Yield control back to the executor to allow other tasks to run
            tokio::task::yield_now().await;
        }
    }
}

// Re-export commonly used types
// Removed duplicate exports to avoid conflicts
