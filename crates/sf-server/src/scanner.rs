//! Library scanner background task.
//!
//! Walks library directories recursively, filters by configured extensions,
//! probes discovered files, and registers them as items + media_files.
//! Optionally queues processing jobs when `auto_convert_on_scan` is enabled.
//!
//! Uses a two-pass approach: source files are ingested first (creating items),
//! then existing conversions (`-pb` files) are linked to their parent items.
//! Profile classification is always based on probed media properties, not
//! filename conventions.

use std::path::{Path, PathBuf};

use sf_core::events::{EventCategory, EventPayload};

use crate::context::AppContext;

/// Interval (in files discovered) between progress event broadcasts.
const PROGRESS_INTERVAL: u64 = 50;

/// Common media file extensions used when searching for a converted file's
/// corresponding source.
const SOURCE_EXTENSIONS: &[&str] = &["mkv", "mp4", "avi", "m4v", "webm", "ts", "wmv", "flv"];

/// Scan a library's directories and register discovered media files.
///
/// For each file: parses the filename for metadata, probes for media info,
/// creates an item + media_file record (skipping duplicates), and optionally
/// queues a conversion job if `auto_convert_on_scan` is enabled.
///
/// Files with a `-pb` suffix are handled in a second pass after all other
/// files have been ingested, so they can be linked to the correct parent item
/// rather than creating duplicates.
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

    let auto_convert = ctx.config_store.conversion.read().auto_convert_on_scan;

    let mut files_found: u64 = 0;
    let mut files_queued: u64 = 0;
    let mut files_skipped: u64 = 0;
    let mut errors: u64 = 0;

    // Collect -pb files for second pass (to link to parent items).
    let mut deferred_pb_files: Vec<PathBuf> = Vec::new();

    // --- Pass 1: primary files ---
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
            .filter_entry(|e| {
                // Skip HLS output directories (e.g. movie-pb.hls/) — they
                // contain .m4s segments and .m3u8 playlists, not scannable media.
                if e.file_type().is_dir() {
                    if let Some(name) = e.file_name().to_str() {
                        if name.ends_with(".hls") {
                            return false;
                        }
                    }
                }
                true
            })
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
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // Skip partial download files.
            if file_name.ends_with(".aria2")
                || file_name.ends_with(".part")
                || file_name.ends_with(".crdownload")
                || file_name.ends_with(".tmp")
            {
                continue;
            }

            // Defer -pb suffixed files to second pass so they link to
            // parent items instead of creating duplicates.
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if stem.ends_with("-pb") {
                    deferred_pb_files.push(path.to_path_buf());
                    continue;
                }
            }

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

    // --- Pass 2: -pb converted files (link to parent items) ---
    for pb_path in &deferred_pb_files {
        let file_path_str = pb_path.to_string_lossy().to_string();
        files_found += 1;

        // Skip if already registered.
        let already_exists = match sf_db::pool::get_conn(&ctx.db) {
            Ok(conn) => {
                match sf_db::queries::media_files::get_media_file_by_path(&conn, &file_path_str) {
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
            continue;
        }

        match ingest_converted_file(&ctx, pb_path).await {
            Ok(true) => {
                tracing::debug!(file = %file_path_str, "Scanner linked converted file to item");
            }
            Ok(false) => {
                tracing::warn!(
                    file = %file_path_str,
                    "No source item found for converted file, skipping"
                );
                files_skipped += 1;
            }
            Err(e) => {
                tracing::warn!(
                    file = %file_path_str, error = %e,
                    "Failed to ingest converted file"
                );
                errors += 1;
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
///
/// The profile and role are determined from the probed media properties,
/// not the filename.
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

    // Classify profile from actual media properties.
    let profile = media_info.classify_profile();
    let role = if profile == sf_core::Profile::B {
        "universal"
    } else {
        "source"
    };

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

    // Create the media_file record with detected profile.
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
        role,
        &profile.to_string(),
        duration_secs,
    )?;

    // Populate HLS cache for Profile B files.
    if profile == sf_core::Profile::B {
        if let Err(e) = crate::hls_prep::populate_hls_cache(ctx, mf.id, path).await {
            tracing::warn!(
                file = %file_path_str, error = %e,
                "Failed to populate HLS cache during scan"
            );
        }
    }

    // Only queue conversion for files that aren't already Profile B.
    if auto_convert && profile != sf_core::Profile::B {
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

/// Ingest a converted file (`-pb` suffix) by linking it to its source item.
///
/// Finds the source media_file by looking for common extensions in the same
/// directory with the stem stripped of the `-pb` suffix. The profile is
/// determined from the probed media properties.
///
/// Returns `Ok(true)` if the file was linked, `Ok(false)` if no source item
/// was found.
async fn ingest_converted_file(ctx: &AppContext, path: &Path) -> sf_core::Result<bool> {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let source_stem = match stem.strip_suffix("-pb") {
        Some(s) => s,
        None => return Ok(false),
    };

    let parent_dir = path.parent().unwrap_or_else(|| Path::new("."));

    // Find the source media_file to get the parent item_id.
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let mut source_item_id = None;

    for ext in SOURCE_EXTENSIONS {
        let source_path = parent_dir.join(format!("{source_stem}.{ext}"));
        let source_path_str = source_path.to_string_lossy();
        if let Some(mf) =
            sf_db::queries::media_files::get_media_file_by_path(&conn, &source_path_str)?
        {
            source_item_id = Some(mf.item_id);
            break;
        }
    }

    let item_id = match source_item_id {
        Some(id) => id,
        None => return Ok(false),
    };

    // Probe the file for actual media properties.
    let media_info = ctx.prober.probe(path)?;
    let profile = media_info.classify_profile();
    let role = if profile == sf_core::Profile::B {
        "universal"
    } else {
        "source"
    };

    let video = media_info.primary_video();
    let audio = media_info.primary_audio();

    let file_path_str = path.to_string_lossy().to_string();
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
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

    let mf = sf_db::queries::media_files::create_media_file(
        &conn,
        item_id,
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
        role,
        &profile.to_string(),
        duration_secs,
    )?;

    // Populate HLS cache for Profile B files.
    if profile == sf_core::Profile::B {
        if let Err(e) = crate::hls_prep::populate_hls_cache(ctx, mf.id, path).await {
            tracing::warn!(
                file = %file_path_str, error = %e,
                "Failed to populate HLS cache for converted file"
            );
        }
    }

    Ok(true)
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
