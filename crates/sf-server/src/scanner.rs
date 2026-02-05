//! Library scanner background task.
//!
//! Walks library directories recursively, filters by configured extensions,
//! probes discovered files, and registers them as items + media_files.
//! Optionally queues processing jobs when `auto_convert_on_scan` is enabled.

use std::path::Path;

use sf_core::events::{EventCategory, EventPayload};

use crate::context::AppContext;

/// Interval (in files discovered) between progress event broadcasts.
const PROGRESS_INTERVAL: u64 = 50;

/// Scan a library's directories and register discovered media files.
///
/// For each file: parses the filename for metadata, probes for media info,
/// creates an item + media_file record (skipping duplicates), and optionally
/// queues a conversion job if `auto_convert_on_scan` is enabled.
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

    let auto_convert = ctx.config.conversion.auto_convert_on_scan;

    let mut files_found: u64 = 0;
    let mut files_queued: u64 = 0;
    let mut files_skipped: u64 = 0;
    let mut errors: u64 = 0;

    for dir in &library.paths {
        let dir_path = Path::new(dir);
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

            // Skip if this file is already registered as a media_file.
            let already_exists = match sf_db::pool::get_conn(&ctx.db) {
                Ok(conn) => {
                    match sf_db::queries::media_files::get_media_file_by_path(&conn, &file_path_str)
                    {
                        Ok(Some(_)) => true,
                        Ok(None) => false,
                        Err(e) => {
                            tracing::warn!(
                                file = %file_path_str, error = %e,
                                "Failed to check existing media_file, skipping"
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

            if already_exists {
                files_skipped += 1;
                emit_progress_if_needed(
                    &ctx,
                    library_id,
                    files_found,
                    files_queued,
                );
                continue;
            }

            // Ingest: probe + register.
            match ingest_file(&ctx, library_id, path, auto_convert).await {
                Ok(queued) => {
                    if queued {
                        files_queued += 1;
                    }
                    tracing::debug!(file = %file_path_str, "Scanner ingested file");
                }
                Err(e) => {
                    tracing::warn!(
                        file = %file_path_str, error = %e,
                        "Failed to ingest file"
                    );
                    errors += 1;
                }
            }

            // Emit progress at intervals.
            emit_progress_if_needed(&ctx, library_id, files_found, files_queued);

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

/// Probe a file, create an item + media_file record, and optionally queue
/// a conversion job. Returns `Ok(true)` if a conversion was queued.
async fn ingest_file(
    ctx: &AppContext,
    library_id: sf_core::LibraryId,
    path: &Path,
    auto_convert: bool,
) -> sf_core::Result<bool> {
    let file_path_str = path.to_string_lossy().to_string();
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let file_stem = path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or(&file_name)
        .to_string();

    // Parse the filename for metadata.
    let parsed = sf_parser::parse(&file_stem);

    // Probe the file for media info.
    let media_info = ctx.prober.probe(path)?;

    let video = media_info.primary_video();
    let audio = media_info.primary_audio();

    let file_size = std::fs::metadata(path)
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    let container = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    let video_codec = video.map(|v| format!("{}", v.codec));
    let audio_codec = audio.map(|a| format!("{}", a.codec));
    let resolution_width = video.map(|v| v.width as i32);
    let resolution_height = video.map(|v| v.height as i32);
    let hdr_format = video.and_then(|v| {
        if v.hdr_format == sf_core::HdrFormat::Sdr {
            None
        } else {
            Some(format!("{}", v.hdr_format))
        }
    });
    let has_dv = video.map_or(false, |v| v.dolby_vision.is_some());
    let dv_profile = video
        .and_then(|v| v.dolby_vision.as_ref())
        .map(|dv| dv.profile as i32);
    let duration_secs = media_info.duration.map(|d| d.as_secs_f64());

    let conn = sf_db::pool::get_conn(&ctx.db)?;

    // Create an item for this file.
    let item = sf_db::queries::items::create_item(
        &conn,
        library_id,
        "movie",
        &parsed.title,
        None,
        parsed.year.map(|y| y as i32),
        None, // overview
        None, // runtime_minutes
        None, // community_rating
        None, // provider_ids
        None, // parent_id
        None, // season_number
        None, // episode_number
    )?;

    ctx.event_bus.broadcast(
        EventCategory::User,
        EventPayload::ItemAdded { item_id: item.id },
    );

    // Create the media_file record.
    let mf = sf_db::queries::media_files::create_media_file(
        &conn,
        item.id,
        &file_path_str,
        &file_name,
        file_size,
        container.as_deref(),
        video_codec.as_deref(),
        audio_codec.as_deref(),
        resolution_width,
        resolution_height,
        hdr_format.as_deref(),
        has_dv,
        dv_profile,
        "source",
        "C",
        duration_secs,
    )?;

    // Optionally queue a conversion job.
    if auto_convert {
        let job = sf_db::queries::conversion_jobs::create_conversion_job(
            &conn, item.id, mf.id,
        )?;
        ctx.event_bus.broadcast(
            EventCategory::Admin,
            EventPayload::ConversionQueued { job_id: job.id },
        );
        return Ok(true);
    }

    Ok(false)
}

fn emit_progress_if_needed(
    ctx: &AppContext,
    library_id: sf_core::LibraryId,
    files_found: u64,
    files_queued: u64,
) {
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
}
