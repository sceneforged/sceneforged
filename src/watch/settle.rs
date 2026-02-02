use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// Tracks files and determines when they've "settled" (stopped changing)
pub struct FileSettleTracker {
    /// Map of file path to last modification time
    pending: HashMap<PathBuf, Instant>,
    /// How long a file must be unchanged to be considered settled
    settle_duration: Duration,
    /// Channel to send settled files
    settled_tx: mpsc::Sender<PathBuf>,
}

impl FileSettleTracker {
    pub fn new(settle_secs: u64, settled_tx: mpsc::Sender<PathBuf>) -> Self {
        Self {
            pending: HashMap::new(),
            settle_duration: Duration::from_secs(settle_secs),
            settled_tx,
        }
    }

    /// Record that a file was modified
    pub fn file_changed(&mut self, path: PathBuf) {
        self.pending.insert(path, Instant::now());
    }

    /// Check for settled files and send them
    pub async fn check_settled(&mut self) {
        let now = Instant::now();
        let settled: Vec<PathBuf> = self
            .pending
            .iter()
            .filter(|(_, last_change)| now.duration_since(**last_change) >= self.settle_duration)
            .map(|(path, _)| path.clone())
            .collect();

        for path in settled {
            self.pending.remove(&path);
            if let Err(e) = self.settled_tx.send(path.clone()).await {
                tracing::error!("Failed to send settled file: {}", e);
            } else {
                tracing::info!("File settled: {:?}", path);
            }
        }
    }

    /// Remove a file from tracking (e.g., if deleted)
    pub fn remove(&mut self, path: &PathBuf) {
        self.pending.remove(path);
    }
}
