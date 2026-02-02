pub mod settle;

pub use settle::FileSettleTracker;

use crate::config::WatchConfig;
use crate::state::{AppState, JobSource};
use anyhow::{Context, Result};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// File watcher that monitors directories for new media files
pub struct FileWatcher {
    config: WatchConfig,
    state: Arc<AppState>,
    watcher: Option<RecommendedWatcher>,
}

impl FileWatcher {
    pub fn new(config: WatchConfig, state: Arc<AppState>) -> Self {
        Self {
            config,
            state,
            watcher: None,
        }
    }

    /// Start watching configured directories
    pub async fn start(&mut self) -> Result<()> {
        if !self.config.enabled {
            tracing::info!("File watcher is disabled");
            return Ok(());
        }

        if self.config.paths.is_empty() {
            tracing::warn!("No watch paths configured");
            return Ok(());
        }

        let (event_tx, mut event_rx) = mpsc::channel::<PathBuf>(100);
        let (settled_tx, mut settled_rx) = mpsc::channel::<PathBuf>(100);

        // Create settle tracker
        let mut settle_tracker = FileSettleTracker::new(self.config.settle_time_secs, settled_tx);

        // Create file watcher
        let extensions: Vec<String> = self.config.extensions.clone();
        let event_tx_clone = event_tx.clone();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    // Only care about creates and modifications
                    if event.kind.is_create() || event.kind.is_modify() {
                        for path in event.paths {
                            // Filter by extension
                            if let Some(ext) = path.extension() {
                                let ext_str = ext.to_string_lossy().to_lowercase();
                                if extensions.is_empty()
                                    || extensions.iter().any(|e| e.to_lowercase() == ext_str)
                                {
                                    let _ = event_tx_clone.blocking_send(path);
                                }
                            }
                        }
                    }
                }
            },
            Config::default(),
        )
        .context("Failed to create file watcher")?;

        // Watch all configured paths
        for path in &self.config.paths {
            if path.exists() {
                watcher
                    .watch(path, RecursiveMode::Recursive)
                    .with_context(|| format!("Failed to watch path: {:?}", path))?;
                tracing::info!("Watching directory: {:?}", path);
            } else {
                tracing::warn!("Watch path does not exist: {:?}", path);
            }
        }

        self.watcher = Some(watcher);

        // Spawn task to process file events
        let state = self.state.clone();
        tokio::spawn(async move {
            let mut check_interval = tokio::time::interval(Duration::from_secs(5));

            loop {
                tokio::select! {
                    // Handle new file events
                    Some(path) = event_rx.recv() => {
                        tracing::debug!("File event: {:?}", path);
                        settle_tracker.file_changed(path);
                    }

                    // Handle settled files
                    Some(path) = settled_rx.recv() => {
                        if path.exists() && path.is_file() {
                            let watch_path = path.parent()
                                .map(|p| p.display().to_string())
                                .unwrap_or_default();

                            let source = JobSource::Watcher { watch_path };

                            match state.queue_job(path.clone(), source) {
                                Ok(job) => {
                                    tracing::info!("Queued watcher job {} for: {:?}", job.id, path);
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to queue watcher job: {}", e);
                                }
                            }
                        }
                    }

                    // Periodically check for settled files
                    _ = check_interval.tick() => {
                        settle_tracker.check_settled().await;
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop watching
    pub fn stop(&mut self) {
        self.watcher = None;
        tracing::info!("File watcher stopped");
    }
}

/// Check if a file has a media extension
pub fn is_media_file(path: &std::path::Path, extensions: &[String]) -> bool {
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();

        if extensions.is_empty() {
            // Default media extensions
            let default_exts = ["mkv", "mp4", "avi", "mov", "wmv", "m4v", "ts", "m2ts"];
            return default_exts.contains(&ext_str.as_str());
        }

        return extensions.iter().any(|e| e.to_lowercase() == ext_str);
    }
    false
}
