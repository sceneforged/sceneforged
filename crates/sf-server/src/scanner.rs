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
//!
//! Performance optimizations:
//! - Batch existence check via HashSet (one query upfront vs per-file)
//! - In-memory series/season cache to avoid redundant DB lookups
//! - Parallel file probing with bounded concurrency
//! - Batched DB writes in transactions (flush every N files)
//! - TMDB enrichment decoupled to background workers

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use sf_core::events::{EventCategory, EventPayload};

use crate::context::AppContext;

/// Interval (in files discovered) between progress event broadcasts.
const PROGRESS_INTERVAL: u64 = 50;

/// Number of files to accumulate before flushing a batch DB write.
const DB_BATCH_SIZE: usize = 50;

/// Maximum number of concurrent probe tasks.
const PROBE_CONCURRENCY: usize = 4;

/// Number of concurrent TMDB enrichment workers.
const ENRICH_CONCURRENCY: usize = 4;

/// Common media file extensions used when searching for a converted file's
/// corresponding source.
const SOURCE_EXTENSIONS: &[&str] = &["mkv", "mp4", "avi", "m4v", "webm", "ts", "wmv", "flv"];

/// Collected probe data for a single file, ready for DB insertion.
struct ProbeResult {
    path: PathBuf,
    file_path_str: String,
    file_name: String,
    parsed: sf_parser::ParsedRelease,
    media_info: sf_probe::types::MediaInfo,
    file_size: i64,
}

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

    // --- Batch existence check: load all known paths into a HashSet ---
    let known_paths: HashSet<String> = match sf_db::pool::get_conn(&ctx.db) {
        Ok(conn) => {
            match sf_db::queries::media_files::list_media_file_paths_for_library(&conn, library_id) {
                Ok(paths) => paths.into_iter().collect(),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to load known paths, falling back to per-file checks");
                    HashSet::new()
                }
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to get DB connection for known paths");
            HashSet::new()
        }
    };

    // --- Series/season cache to avoid redundant DB lookups ---
    let mut series_cache: HashMap<(sf_core::LibraryId, String), sf_db::models::Item> =
        HashMap::new();
    // Key: (series_id, season_number)
    let mut season_cache: HashMap<(sf_core::ItemId, i32), sf_db::models::Item> = HashMap::new();

    // --- TMDB enrichment channel ---
    let (enrich_tx, enrich_rx) = tokio::sync::mpsc::channel::<(sf_core::ItemId, AppContext)>(256);
    let enrich_rx = std::sync::Arc::new(tokio::sync::Mutex::new(enrich_rx));
    let mut enrich_handles = Vec::new();
    for _ in 0..ENRICH_CONCURRENCY {
        let rx = enrich_rx.clone();
        enrich_handles.push(tokio::spawn(async move {
            loop {
                let msg = {
                    let mut guard = rx.lock().await;
                    guard.recv().await
                };
                match msg {
                    Some((item_id, ctx)) => auto_enrich(&ctx, item_id).await,
                    None => break,
                }
            }
        }));
    }

    // Collect -pb files for second pass (to link to parent items).
    let mut deferred_pb_files: Vec<PathBuf> = Vec::new();

    // --- Pass 1: Walk + parallel probe + batched DB writes ---
    // Collect files to probe.
    let mut files_to_probe: Vec<PathBuf> = Vec::new();

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

            // Batch existence check via HashSet.
            if known_paths.contains(&file_path_str) {
                files_skipped += 1;
                emit_progress_if_needed(
                    &ctx,
                    library_id,
                    files_found,
                    files_queued,
                );
                continue;
            }

            files_to_probe.push(path.to_path_buf());

            // Emit progress at intervals.
            emit_progress_if_needed(&ctx, library_id, files_found, files_queued);
        }
    }

    // --- Parallel probe phase ---
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(PROBE_CONCURRENCY));
    let mut probe_handles = Vec::new();

    for file_path in files_to_probe {
        let sem = semaphore.clone();
        let prober = ctx.prober.clone();

        probe_handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.expect("semaphore closed");

            let path = file_path.clone();
            let result = tokio::task::spawn_blocking(move || {
                let info = prober.probe(&path)?;
                let size = std::fs::metadata(&path)
                    .map(|m| m.len() as i64)
                    .unwrap_or(0);
                Ok::<_, sf_core::Error>((info, size))
            })
            .await;

            match result {
                Ok(Ok((media_info, file_size))) => {
                    let file_name = file_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let file_stem = file_path
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&file_name)
                        .to_string();
                    let parsed = sf_parser::parse(&file_stem);
                    let file_path_str = file_path.to_string_lossy().to_string();

                    Ok(ProbeResult {
                        path: file_path,
                        file_path_str,
                        file_name,
                        parsed,
                        media_info,
                        file_size,
                    })
                }
                Ok(Err(e)) => Err((file_path, e)),
                Err(e) => Err((
                    file_path,
                    sf_core::Error::Io {
                        source: std::io::Error::new(std::io::ErrorKind::Other, e),
                    },
                )),
            }
        }));
    }

    // Collect probe results and write to DB in batches.
    let mut batch: Vec<ProbeResult> = Vec::with_capacity(DB_BATCH_SIZE);

    for handle in probe_handles {
        match handle.await {
            Ok(Ok(result)) => {
                batch.push(result);
                if batch.len() >= DB_BATCH_SIZE {
                    let (queued, errs) = flush_batch(
                        &ctx,
                        library_id,
                        auto_convert,
                        &mut batch,
                        &enrich_tx,
                        &mut series_cache,
                        &mut season_cache,
                    )
                    .await;
                    files_queued += queued;
                    errors += errs;
                }
            }
            Ok(Err((path, e))) => {
                tracing::warn!(file = %path.display(), error = %e, "Failed to probe file");
                errors += 1;
            }
            Err(e) => {
                tracing::warn!(error = %e, "Probe task panicked");
                errors += 1;
            }
        }
    }

    // Flush remaining batch.
    if !batch.is_empty() {
        let (queued, errs) = flush_batch(
            &ctx,
            library_id,
            auto_convert,
            &mut batch,
            &enrich_tx,
            &mut series_cache,
            &mut season_cache,
        )
        .await;
        files_queued += queued;
        errors += errs;
    }

    // --- Pass 2: -pb converted files (link to parent items) ---
    for pb_path in &deferred_pb_files {
        let file_path_str = pb_path.to_string_lossy().to_string();
        files_found += 1;

        // Batch existence check.
        if known_paths.contains(&file_path_str) {
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

    // --- Wait for TMDB enrichment workers to finish ---
    drop(enrich_tx);
    for handle in enrich_handles {
        let _ = handle.await;
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

    // Release the scan lock so the library can be scanned again.
    ctx.active_scans.remove(&library_id);

    tracing::info!(
        library_id = %library_id,
        files_found,
        files_queued,
        files_skipped,
        errors,
        "Library scan complete"
    );
}

/// Flush a batch of probe results to the database in a single transaction.
///
/// Returns (files_queued, errors).
async fn flush_batch(
    ctx: &AppContext,
    library_id: sf_core::LibraryId,
    auto_convert: bool,
    batch: &mut Vec<ProbeResult>,
    enrich_tx: &tokio::sync::mpsc::Sender<(sf_core::ItemId, AppContext)>,
    series_cache: &mut HashMap<(sf_core::LibraryId, String), sf_db::models::Item>,
    season_cache: &mut HashMap<(sf_core::ItemId, i32), sf_db::models::Item>,
) -> (u64, u64) {
    let mut files_queued: u64 = 0;
    let mut errors: u64 = 0;

    // Items to enrich after the transaction commits.
    let mut enrich_items: Vec<sf_core::ItemId> = Vec::new();
    // HLS cache tasks to run after the transaction commits.
    let mut hls_tasks: Vec<(sf_core::MediaFileId, PathBuf)> = Vec::new();

    // Run the synchronous transaction in a block so the non-Send Connection
    // is dropped before any .await points.
    {
        let conn = match sf_db::pool::get_conn(&ctx.db) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to get DB connection for batch flush");
                errors += batch.len() as u64;
                batch.clear();
                return (files_queued, errors);
            }
        };

        let tx = match conn.unchecked_transaction() {
            Ok(tx) => tx,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to start transaction for batch flush");
                errors += batch.len() as u64;
                batch.clear();
                return (files_queued, errors);
            }
        };

        for result in batch.drain(..) {
            match ingest_probed_file(
                ctx,
                &tx,
                library_id,
                auto_convert,
                result,
                series_cache,
                season_cache,
            ) {
                Ok(IngestOutcome {
                    queued,
                    enrich_item_id,
                    mf_id,
                    is_profile_b,
                    path,
                }) => {
                    if queued {
                        files_queued += 1;
                    }
                    if let Some(eid) = enrich_item_id {
                        enrich_items.push(eid);
                    }
                    if is_profile_b {
                        hls_tasks.push((mf_id, path));
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to ingest file in batch");
                    errors += 1;
                }
            }
        }

        if let Err(e) = tx.commit() {
            tracing::warn!(error = %e, "Failed to commit batch transaction");
            return (0, errors);
        }
    }
    // conn and tx are now dropped — safe to .await below.

    // Post-commit: send enrichment requests.
    for eid in enrich_items {
        let _ = enrich_tx.send((eid, ctx.clone())).await;
    }

    // Post-commit: populate HLS cache for Profile B files.
    for (mf_id, path) in hls_tasks {
        if let Err(e) = crate::hls_prep::populate_hls_cache(ctx, mf_id, &path).await {
            tracing::warn!(
                file = %path.display(), error = %e,
                "Failed to populate HLS cache during scan"
            );
        }
    }

    (files_queued, errors)
}

struct IngestOutcome {
    queued: bool,
    enrich_item_id: Option<sf_core::ItemId>,
    mf_id: sf_core::MediaFileId,
    is_profile_b: bool,
    path: PathBuf,
}

/// Ingest a single probed file into the database (within an existing transaction).
fn ingest_probed_file(
    ctx: &AppContext,
    conn: &rusqlite::Connection,
    library_id: sf_core::LibraryId,
    auto_convert: bool,
    result: ProbeResult,
    series_cache: &mut HashMap<(sf_core::LibraryId, String), sf_db::models::Item>,
    season_cache: &mut HashMap<(sf_core::ItemId, i32), sf_db::models::Item>,
) -> sf_core::Result<IngestOutcome> {
    let ProbeResult {
        path,
        file_path_str,
        file_name,
        parsed,
        media_info,
        file_size,
    } = result;

    let profile = media_info.classify_profile();
    let role = if profile == sf_core::Profile::B {
        "universal"
    } else {
        "source"
    };

    let video = media_info.primary_video();
    let audio = media_info.primary_audio();

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

    // Create item(s) — for TV episodes we need series → season → episode hierarchy.
    let item = if parsed.season.is_some() && parsed.episode.is_some() {
        let season_num = parsed.season.unwrap() as i32;
        let episode_num = parsed.episode.unwrap() as i32;

        // Use series cache to avoid redundant DB lookups.
        let cache_key = (library_id, parsed.title.clone());
        let series = if let Some(s) = series_cache.get(&cache_key) {
            s.clone()
        } else {
            let s = sf_db::queries::items::find_or_create_series(
                conn,
                library_id,
                &parsed.title,
                parsed.year.map(|y| y as i32),
            )?;
            series_cache.insert(cache_key, s.clone());
            s
        };

        // Use season cache.
        let season_key = (series.id, season_num);
        let season = if let Some(s) = season_cache.get(&season_key) {
            s.clone()
        } else {
            let s = sf_db::queries::items::find_or_create_season(
                conn,
                library_id,
                series.id,
                season_num,
            )?;
            season_cache.insert(season_key, s.clone());
            s
        };

        // Build episode name: "S01E01" or "S01E01E02" for multi-ep.
        let ep_name = if let Some(end) = parsed.episode_end {
            format!(
                "{} S{:02}E{:02}E{:02}",
                parsed.title, season_num, episode_num, end
            )
        } else {
            format!("{} S{:02}E{:02}", parsed.title, season_num, episode_num)
        };

        sf_db::queries::items::create_item(
            conn,
            library_id,
            "episode",
            &ep_name,
            None,
            parsed.year.map(|y| y as i32),
            None,
            None,
            None,
            None,
            Some(season.id),
            Some(season_num),
            Some(episode_num),
        )?
    } else {
        sf_db::queries::items::create_item(
            conn,
            library_id,
            "movie",
            &parsed.title,
            None,
            parsed.year.map(|y| y as i32),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )?
    };

    ctx.event_bus.broadcast(
        EventCategory::User,
        EventPayload::ItemAdded { item_id: item.id },
    );

    // Determine which item to enrich (series for episodes, self for movies).
    let enrich_item_id = if item.item_kind == "episode" {
        // Walk up to find series: episode → season → series.
        item.parent_id.and_then(|season_id| {
            sf_db::queries::items::get_item(conn, season_id)
                .ok()
                .flatten()
                .and_then(|s| s.parent_id)
        })
    } else {
        Some(item.id)
    };

    // Create the media_file record with detected profile.
    let mf = sf_db::queries::media_files::create_media_file(
        conn,
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

    // Store subtitle tracks from probe data.
    for (idx, sub) in media_info.subtitle_tracks.iter().enumerate() {
        if let Err(e) = sf_db::queries::subtitle_tracks::create_subtitle_track(
            conn,
            mf.id,
            idx as i32,
            &sub.codec,
            sub.language.as_deref(),
            sub.forced,
            sub.default,
        ) {
            tracing::warn!(error = %e, "Failed to store subtitle track {idx}");
        }
    }

    let is_profile_b = profile == sf_core::Profile::B;
    let mut queued = false;

    // Only queue conversion for files that aren't already Profile B.
    if auto_convert && !is_profile_b {
        let job = sf_db::queries::conversion_jobs::create_conversion_job(conn, item.id, mf.id)?;
        ctx.event_bus.broadcast(
            EventCategory::Admin,
            EventPayload::ConversionQueued { job_id: job.id },
        );
        queued = true;
    }

    tracing::debug!(file = %file_path_str, "Scanner ingested file");

    Ok(IngestOutcome {
        queued,
        enrich_item_id,
        mf_id: mf.id,
        is_profile_b,
        path,
    })
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

    // Probe the file for actual media properties — run in spawn_blocking.
    let prober = ctx.prober.clone();
    let probe_path = path.to_path_buf();
    let (media_info, file_size) = tokio::task::spawn_blocking(move || {
        let info = prober.probe(&probe_path)?;
        let size = std::fs::metadata(&probe_path)
            .map(|m| m.len() as i64)
            .unwrap_or(0);
        Ok::<_, sf_core::Error>((info, size))
    })
    .await
    .map_err(|e| sf_core::Error::Io {
        source: std::io::Error::new(std::io::ErrorKind::Other, e),
    })??;

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

/// Best-effort TMDB enrichment for a single item during scan.
/// Silently returns on any failure (missing API key, no results, network error).
async fn auto_enrich(ctx: &AppContext, item_id: sf_core::ItemId) {
    let (api_key, language) = {
        let meta = ctx.config_store.metadata.read();
        if !meta.auto_enrich {
            return;
        }
        let api_key = match meta.tmdb_api_key.clone() {
            Some(k) if !k.is_empty() => k,
            _ => return,
        };
        (api_key, meta.language.clone())
    };

    // Check if already enriched (has provider_ids with tmdb key).
    let item = match sf_db::pool::get_conn(&ctx.db)
        .ok()
        .and_then(|c| sf_db::queries::items::get_item(&c, item_id).ok().flatten())
    {
        Some(i) => i,
        None => return,
    };
    if item.provider_ids.contains("\"tmdb\"") {
        return; // Already enriched.
    }

    let client = crate::tmdb::TmdbClient::new(api_key, language);
    let is_tv = item.item_kind == "series";

    let results = if is_tv {
        client
            .search_tv(&item.name, item.year.map(|y| y as u32))
            .await
    } else {
        client
            .search_movie(&item.name, item.year.map(|y| y as u32))
            .await
    };

    let results = match results {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(item_id = %item_id, error = %e, "Auto-enrich TMDB search failed");
            return;
        }
    };

    let tmdb_id = match results.first() {
        Some(r) => r.id,
        None => return,
    };

    // Use the enrich_item_with_body helper to do the actual enrichment.
    if crate::routes::metadata::enrich_item_with_body(
        ctx.clone(),
        item_id.to_string(),
        crate::routes::metadata::EnrichRequest {
            tmdb_id: Some(tmdb_id),
            media_type: if is_tv {
                Some("tv".into())
            } else {
                Some("movie".into())
            },
        },
    )
    .await
    .is_err()
    {
        tracing::debug!(item_id = %item_id, "Auto-enrich failed");
    }
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
