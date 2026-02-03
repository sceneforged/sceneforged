//! File watcher background task.
//!
//! Watches configured directories for new files matching configured
//! extensions, with a settle time to wait for writes to complete before
//! queuing jobs.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

use crate::context::AppContext;

/// Start the file watcher background task.
///
/// Watches the directories specified in config, queues jobs for new files
/// after they have settled (no further writes for `settle_time`).
pub async fn run_watcher(ctx: AppContext, cancel: CancellationToken) {
    let watch_config = &ctx.config.watch;

    if !watch_config.enabled || watch_config.paths.is_empty() {
        tracing::info!("File watcher disabled or no paths configured");
        return;
    }

    let settle_time = Duration::from_secs(watch_config.settle_time_secs);
    let extensions: Vec<String> = watch_config
        .extensions
        .iter()
        .map(|e| e.to_lowercase())
        .collect();

    // Track files that are settling (path -> last seen time).
    let pending: Arc<Mutex<HashMap<PathBuf, Instant>>> = Arc::new(Mutex::new(HashMap::new()));
    let pending_clone = pending.clone();

    // Create the notify watcher.
    let (_tx, mut rx) = tokio::sync::mpsc::channel::<PathBuf>(256);

    let mut watcher: RecommendedWatcher = match notify::recommended_watcher(
        move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) => {
                        for path in event.paths {
                            let mut map = pending_clone.lock();
                            map.insert(path, Instant::now());
                        }
                    }
                    _ => {}
                }
            }
        },
    ) {
        Ok(w) => w,
        Err(e) => {
            tracing::error!("Failed to create file watcher: {e}");
            return;
        }
    };

    // Watch configured paths.
    for path in &watch_config.paths {
        if path.exists() {
            if let Err(e) = watcher.watch(path, RecursiveMode::Recursive) {
                tracing::warn!("Failed to watch {}: {e}", path.display());
            } else {
                tracing::info!("Watching directory: {}", path.display());
            }
        } else {
            tracing::warn!("Watch path does not exist: {}", path.display());
        }
    }

    // Spawn a task that periodically checks for settled files.
    let ctx_clone = ctx.clone();
    let cancel_clone = cancel.clone();
    let extensions_clone = extensions.clone();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(1)) => {}
                _ = cancel_clone.cancelled() => { break; }
            }

            let now = Instant::now();
            let mut settled = Vec::new();

            {
                let mut map = pending.lock();
                map.retain(|path, last_seen| {
                    if now.duration_since(*last_seen) >= settle_time {
                        settled.push(path.clone());
                        false
                    } else {
                        true
                    }
                });
            }

            for path in settled {
                // Check extension filter.
                if !extensions_clone.is_empty() {
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase())
                        .unwrap_or_default();
                    if !extensions_clone.contains(&ext) {
                        continue;
                    }
                }

                // Queue a job for this file.
                let file_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");

                if let Ok(conn) = sf_db::pool::get_conn(&ctx_clone.db) {
                    match sf_db::queries::jobs::create_job(
                        &conn,
                        &path.to_string_lossy(),
                        file_name,
                        Some("watcher"),
                        0,
                    ) {
                        Ok(job) => {
                            tracing::info!(
                                job_id = %job.id,
                                file = %path.display(),
                                "File watcher queued job"
                            );
                            ctx_clone.event_bus.broadcast(
                                sf_core::events::EventCategory::Admin,
                                sf_core::events::EventPayload::JobQueued { job_id: job.id },
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                file = %path.display(),
                                error = %e,
                                "Failed to queue watcher job"
                            );
                        }
                    }
                }
            }
        }
    });

    // Keep the watcher alive and forward settled paths via channel.
    // The main loop just waits for cancellation.
    tokio::select! {
        _ = cancel.cancelled() => {}
        _ = async {
            while let Some(_path) = rx.recv().await {
                // Handled above in the periodic check task.
            }
        } => {}
    }

    tracing::info!("File watcher stopped");
    drop(watcher);
}
