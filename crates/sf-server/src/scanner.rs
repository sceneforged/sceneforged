//! Library scanner background task.
//!
//! Walks library directories recursively, filters by configured extensions,
//! skips files with active jobs, and queues new jobs for discovered files.

use sf_core::events::{EventCategory, EventPayload};

use crate::context::AppContext;

/// Interval (in files discovered) between progress event broadcasts.
const PROGRESS_INTERVAL: u64 = 50;

/// Scan a library's directories and queue jobs for discovered media files.
///
/// Walks all paths in the library recursively, filters by configured extensions,
/// skips files with active jobs, creates jobs for new files, and emits progress
/// and completion events.
pub async fn scan_library(ctx: AppContext, library: sf_db::models::Library) {
    let library_id = library.id;
    let extensions: Vec<String> = ctx
        .config
        .watch
        .extensions
        .iter()
        .map(|e| e.to_lowercase())
        .collect();

    tracing::info!(
        library_id = %library_id,
        paths = ?library.paths,
        extensions = ?extensions,
        "Starting library scan"
    );

    let mut files_found: u64 = 0;
    let mut files_queued: u64 = 0;
    let mut files_skipped: u64 = 0;
    let mut errors: u64 = 0;

    for dir in &library.paths {
        let dir_path = std::path::Path::new(dir);
        if !dir_path.exists() {
            tracing::warn!(path = %dir, "Library scan path does not exist, skipping");
            errors += 1;
            continue;
        }

        for entry in walkdir::WalkDir::new(dir_path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| match e {
                Ok(entry) => Some(entry),
                Err(err) => {
                    tracing::warn!(error = %err, "Error walking directory");
                    None
                }
            })
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();

            // Extension filter.
            if !extensions.is_empty() {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_default();
                if !extensions.contains(&ext) {
                    continue;
                }
            }

            files_found += 1;

            let file_path_str = path.to_string_lossy().to_string();
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            // Dedup: skip if active job exists.
            let should_skip = match sf_db::pool::get_conn(&ctx.db) {
                Ok(conn) => {
                    match sf_db::queries::jobs::has_active_job_for_path(&conn, &file_path_str) {
                        Ok(has_active) => has_active,
                        Err(e) => {
                            tracing::warn!(
                                file = %file_path_str, error = %e,
                                "Failed to check dedup, skipping file"
                            );
                            errors += 1;
                            continue;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to get DB connection during scan");
                    errors += 1;
                    continue;
                }
            };

            if should_skip {
                files_skipped += 1;
                if files_found % PROGRESS_INTERVAL == 0 {
                    ctx.event_bus.broadcast(
                        EventCategory::User,
                        EventPayload::LibraryScanProgress {
                            library_id,
                            files_found,
                            files_queued,
                        },
                    );
                }
                continue;
            }

            // Create job.
            match sf_db::pool::get_conn(&ctx.db) {
                Ok(conn) => {
                    match sf_db::queries::jobs::create_job(
                        &conn,
                        &file_path_str,
                        &file_name,
                        Some("scan"),
                        0,
                    ) {
                        Ok(job) => {
                            files_queued += 1;
                            tracing::debug!(
                                job_id = %job.id,
                                file = %file_path_str,
                                "Scanner queued job"
                            );
                            ctx.event_bus.broadcast(
                                EventCategory::Admin,
                                EventPayload::JobQueued { job_id: job.id },
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                file = %file_path_str, error = %e,
                                "Failed to create scan job"
                            );
                            errors += 1;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to get DB connection for job creation");
                    errors += 1;
                }
            }

            // Emit progress at intervals.
            if files_found % PROGRESS_INTERVAL == 0 {
                ctx.event_bus.broadcast(
                    EventCategory::User,
                    EventPayload::LibraryScanProgress {
                        library_id,
                        files_found,
                        files_queued,
                    },
                );
            }

            // Yield to the runtime periodically to avoid starving other tasks.
            if files_found % 100 == 0 {
                tokio::task::yield_now().await;
            }
        }
    }

    // Emit completion.
    ctx.event_bus.broadcast(
        EventCategory::User,
        EventPayload::LibraryScanComplete {
            library_id,
            files_found,
            files_queued,
            files_skipped,
            errors,
        },
    );

    tracing::info!(
        library_id = %library_id,
        files_found,
        files_queued,
        files_skipped,
        errors,
        "Library scan complete"
    );
}
