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
//! Pipeline architecture:
//!   Walk+Register ──mpsc──> Probe Pool (8) ──mpsc──> DB Writer ──mpsc──> Enrich Pool (4, background)
//!
//! Walk creates pending items immediately so they appear in the browse page.
//! Probers update items to ready/error after probing completes.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use sf_core::events::{EventCategory, EventPayload};

use crate::context::AppContext;

/// Number of files to accumulate before flushing a batch DB write.
const DB_BATCH_SIZE: usize = 50;

/// Maximum number of concurrent probe workers.
const PROBE_CONCURRENCY: usize = 8;

/// Number of concurrent TMDB enrichment workers.
const ENRICH_CONCURRENCY: usize = 4;

/// Maximum concurrent PB file ingestion tasks.
const PB_CONCURRENCY: usize = 4;

/// Interval in milliseconds between progress event emissions.
const PROGRESS_INTERVAL_MS: u64 = 500;

/// Maximum time (ms) to wait for more probe results before flushing a partial batch.
/// This ensures items appear in the UI progressively instead of all at the end.
const DB_FLUSH_TIMEOUT_MS: u64 = 500;

/// Maximum time to wait for a single TMDB enrichment before giving up.
const ENRICH_TIMEOUT_SECS: u64 = 15;

/// Common media file extensions used when searching for a converted file's
/// corresponding source.
const SOURCE_EXTENSIONS: &[&str] = &["mkv", "mp4", "avi", "m4v", "webm", "ts", "wmv", "flv"];

/// Data sent from Walk to Probe: includes the pre-created item_id.
struct WalkResult {
    path: PathBuf,
    file_path_str: String,
    file_name: String,
    _parsed: sf_parser::ParsedRelease,
    item_id: sf_core::ItemId,
}

/// Outcome sent from Probe to DB Writer.
enum ProbeOutcome {
    Success {
        item_id: sf_core::ItemId,
        walk: WalkResult,
        media_info: sf_probe::types::MediaInfo,
        file_size: i64,
    },
    Failure {
        item_id: sf_core::ItemId,
        file_path: String,
        error: String,
    },
}

/// Shared atomic counters for progress reporting across pipeline stages.
struct ScanCounters {
    files_found: AtomicU64,
    files_skipped: AtomicU64,
    files_queued: AtomicU64,
    errors: AtomicU64,
    probes_completed: AtomicU64,
    total_to_probe: AtomicU64,
    items_to_enrich: AtomicU64,
    items_enriched: AtomicU64,
}

impl ScanCounters {
    fn new() -> Self {
        Self {
            files_found: AtomicU64::new(0),
            files_skipped: AtomicU64::new(0),
            files_queued: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            probes_completed: AtomicU64::new(0),
            total_to_probe: AtomicU64::new(0),
            items_to_enrich: AtomicU64::new(0),
            items_enriched: AtomicU64::new(0),
        }
    }
}

/// Scan a library's directories and register discovered media files.
///
/// Uses a streaming pipeline: walk+register → probe → write → enrich.
/// Items are created immediately during the walk phase so they appear in
/// the browse page right away. Enrichment runs in the background after
/// the scan "completes".
pub async fn scan_library(
    ctx: AppContext,
    library: sf_db::models::Library,
    cancel_token: CancellationToken,
) {
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

    let counters = Arc::new(ScanCounters::new());

    // --- Channels ---
    // Walk → Probe: walk results with pre-created item_ids
    let (walk_tx, walk_rx) = tokio::sync::mpsc::channel::<WalkResult>(256);
    // Walk → PB collector: deferred -pb files
    let (pb_tx, mut pb_rx) = tokio::sync::mpsc::channel::<PathBuf>(256);
    // Probe → DB Writer: probe outcomes
    let (probe_tx, mut probe_rx) = tokio::sync::mpsc::channel::<ProbeOutcome>(64);
    // DB Writer → Enrichment: items to enrich
    let (enrich_tx, enrich_rx) = tokio::sync::mpsc::channel::<(sf_core::ItemId, sf_core::LibraryId, AppContext)>(256);
    let enrich_rx = Arc::new(tokio::sync::Mutex::new(enrich_rx));

    // --- Phase tracking ---
    let (phase_tx, phase_rx) = tokio::sync::watch::channel("walking".to_string());

    // --- Spawn enrichment workers (run in background after scan completes) ---
    let mut enrich_handles = Vec::new();
    for _ in 0..ENRICH_CONCURRENCY {
        let rx = enrich_rx.clone();
        let enrich_counters = counters.clone();
        let enrich_cancel = cancel_token.clone();
        enrich_handles.push(tokio::spawn(async move {
            loop {
                let msg = {
                    let mut guard = rx.lock().await;
                    guard.recv().await
                };
                match msg {
                    Some((item_id, library_id, ctx)) => {
                        if enrich_cancel.is_cancelled() {
                            enrich_counters.items_enriched.fetch_add(1, Ordering::Relaxed);
                            continue;
                        }
                        // Timeout enrichment to prevent blocking.
                        match tokio::time::timeout(
                            Duration::from_secs(ENRICH_TIMEOUT_SECS),
                            auto_enrich(&ctx, item_id, library_id),
                        )
                        .await
                        {
                            Ok(()) => {}
                            Err(_) => {
                                tracing::warn!(item_id = %item_id, "Enrichment timed out after {}s", ENRICH_TIMEOUT_SECS);
                                // Emit enriched to clear the spinner.
                                ctx.event_bus.broadcast(
                                    EventCategory::User,
                                    EventPayload::ItemEnriched { item_id, library_id },
                                );
                            }
                        }
                        enrich_counters.items_enriched.fetch_add(1, Ordering::Relaxed);
                    }
                    None => break,
                }
            }
        }));
    }

    // --- Spawn walk stage (blocking I/O + item creation) ---
    let walk_ctx = ctx.clone();
    let walk_counters = counters.clone();
    let walk_known_paths = known_paths.clone();
    let walk_extensions = extensions;
    let walk_paths = library.paths.clone();
    let walk_library_id = library_id;
    let walk_cancel = cancel_token.clone();

    let walk_handle = tokio::task::spawn_blocking(move || {
        let mut deferred_count: u64 = 0;

        // Series/season caches for item creation during walk.
        let mut series_cache: HashMap<(sf_core::LibraryId, String), sf_db::models::Item> =
            HashMap::new();
        let mut season_cache: HashMap<(sf_core::ItemId, i32), sf_db::models::Item> =
            HashMap::new();

        // Get a DB connection for creating items during walk.
        let conn = match sf_db::pool::get_conn(&walk_ctx.db) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(error = %e, "Failed to get DB connection for walk phase");
                return deferred_count;
            }
        };

        for dir in &walk_paths {
            if walk_cancel.is_cancelled() {
                break;
            }

            let dir_path = Path::new(dir);
            if !dir_path.exists() {
                tracing::warn!(path = %dir, "Library scan path does not exist, skipping");
                walk_counters.errors.fetch_add(1, Ordering::Relaxed);
                walk_ctx.event_bus.broadcast(
                    EventCategory::User,
                    EventPayload::LibraryScanError {
                        library_id: walk_library_id,
                        file_path: dir.clone(),
                        message: "Path does not exist".into(),
                    },
                );
                continue;
            }

            for entry in walkdir::WalkDir::new(dir_path)
                .follow_links(true)
                .into_iter()
                .filter_entry(|e| {
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
                if walk_cancel.is_cancelled() {
                    break;
                }

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

                // Defer -pb suffixed files to second pass.
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if stem.ends_with("-pb") {
                        let _ = pb_tx.blocking_send(path.to_path_buf());
                        deferred_count += 1;
                        continue;
                    }
                }

                // Extension filter.
                if !walk_extensions.is_empty() {
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase())
                        .unwrap_or_default();
                    if !walk_extensions.contains(&ext) {
                        continue;
                    }
                }

                walk_counters.files_found.fetch_add(1, Ordering::Relaxed);

                let file_path_str = path.to_string_lossy().to_string();

                // Batch existence check via HashSet.
                if walk_known_paths.contains(&file_path_str) {
                    walk_counters.files_skipped.fetch_add(1, Ordering::Relaxed);
                    continue;
                }

                walk_counters.total_to_probe.fetch_add(1, Ordering::Relaxed);

                // Parse filename for item creation.
                let file_name_str = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let file_stem = path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&file_name_str)
                    .to_string();
                let parsed = sf_parser::parse(&file_stem);

                // Create pending item in DB immediately.
                let item_id = match create_pending_item_for_walk(
                    &walk_ctx,
                    &conn,
                    walk_library_id,
                    &parsed,
                    &file_path_str,
                    &mut series_cache,
                    &mut season_cache,
                ) {
                    Ok(id) => id,
                    Err(e) => {
                        tracing::warn!(error = %e, file = %file_path_str, "Failed to create pending item");
                        walk_counters.errors.fetch_add(1, Ordering::Relaxed);
                        walk_ctx.event_bus.broadcast(
                            EventCategory::User,
                            EventPayload::LibraryScanError {
                                library_id: walk_library_id,
                                file_path: file_path_str,
                                message: format!("Item creation failed: {e}"),
                            },
                        );
                        continue;
                    }
                };

                let walk_result = WalkResult {
                    path: path.to_path_buf(),
                    file_path_str,
                    file_name: file_name_str,
                    _parsed: parsed,
                    item_id,
                };

                // Send to probe pool — blocking_send provides backpressure.
                if walk_tx.blocking_send(walk_result).is_err() {
                    break; // Receiver dropped, scan cancelled.
                }
            }
        }

        deferred_count
    });

    // --- Spawn probe pool (work-stealing via shared receiver) ---
    let walk_rx = Arc::new(tokio::sync::Mutex::new(walk_rx));
    let mut probe_handles = Vec::new();

    for _ in 0..PROBE_CONCURRENCY {
        let rx = walk_rx.clone();
        let tx = probe_tx.clone();
        let prober = ctx.prober.clone();
        let probe_counters = counters.clone();
        let probe_cancel = cancel_token.clone();

        probe_handles.push(tokio::spawn(async move {
            loop {
                let walk_result = {
                    let mut guard = rx.lock().await;
                    guard.recv().await
                };
                let walk_result = match walk_result {
                    Some(wr) => wr,
                    None => break, // Channel closed, walk complete.
                };

                if probe_cancel.is_cancelled() {
                    // Mark as failure on cancellation.
                    let _ = tx
                        .send(ProbeOutcome::Failure {
                            item_id: walk_result.item_id,
                            file_path: walk_result.file_path_str,
                            error: "Scan cancelled".into(),
                        })
                        .await;
                    probe_counters.probes_completed.fetch_add(1, Ordering::Relaxed);
                    continue;
                }

                let prober_clone = prober.clone();
                let path_clone = walk_result.path.clone();
                let result = tokio::task::spawn_blocking(move || {
                    let info = prober_clone.probe(&path_clone)?;
                    let size = std::fs::metadata(&path_clone)
                        .map(|m| m.len() as i64)
                        .unwrap_or(0);
                    Ok::<_, sf_core::Error>((info, size))
                })
                .await;

                probe_counters.probes_completed.fetch_add(1, Ordering::Relaxed);

                let outcome = match result {
                    Ok(Ok((media_info, file_size))) => ProbeOutcome::Success {
                        item_id: walk_result.item_id,
                        walk: walk_result,
                        media_info,
                        file_size,
                    },
                    Ok(Err(e)) => {
                        tracing::warn!(file = %walk_result.file_path_str, error = %e, "Failed to probe file");
                        ProbeOutcome::Failure {
                            item_id: walk_result.item_id,
                            file_path: walk_result.file_path_str,
                            error: format!("Probe failed: {e}"),
                        }
                    }
                    Err(e) => {
                        tracing::warn!(file = %walk_result.file_path_str, error = %e, "Probe task panicked");
                        ProbeOutcome::Failure {
                            item_id: walk_result.item_id,
                            file_path: walk_result.file_path_str,
                            error: format!("Probe task panicked: {e}"),
                        }
                    }
                };

                if tx.send(outcome).await.is_err() {
                    break; // DB writer dropped.
                }
            }
        }));
    }
    // Drop our copy of probe_tx so the DB writer sees EOF when all probers finish.
    drop(probe_tx);

    // --- Spawn DB writer (single consumer — creates media_files, updates scan status) ---
    let writer_ctx = ctx.clone();
    let writer_counters = counters.clone();
    let writer_enrich_tx = enrich_tx.clone();
    let writer_phase_tx = phase_tx.clone();

    let db_writer_handle = tokio::spawn(async move {
        let mut batch: Vec<ProbeOutcome> = Vec::with_capacity(DB_BATCH_SIZE);
        let mut transitioned_to_writing = false;
        let flush_duration = std::time::Duration::from_millis(DB_FLUSH_TIMEOUT_MS);
        let mut batch_deadline: Option<tokio::time::Instant> = None;

        loop {
            let recv_result = if batch.is_empty() {
                probe_rx.recv().await
            } else {
                let deadline = batch_deadline.unwrap();
                match tokio::time::timeout_at(deadline, probe_rx.recv()).await {
                    Ok(result) => result,
                    Err(_) => None,
                }
            };

            match recv_result {
                Some(outcome) => {
                    if batch.is_empty() {
                        batch_deadline = Some(tokio::time::Instant::now() + flush_duration);
                    }
                    batch.push(outcome);
                    if batch.len() >= DB_BATCH_SIZE {
                        if !transitioned_to_writing {
                            let _ = writer_phase_tx.send("writing".to_string());
                            transitioned_to_writing = true;
                        }
                        let (queued, errs) = flush_batch(
                            &writer_ctx,
                            library_id,
                            auto_convert,
                            &mut batch,
                            &writer_enrich_tx,
                            &writer_counters,
                        )
                        .await;
                        writer_counters.files_queued.fetch_add(queued, Ordering::Relaxed);
                        writer_counters.errors.fetch_add(errs, Ordering::Relaxed);
                        batch_deadline = None;
                    }
                }
                None => {
                    if !batch.is_empty() {
                        if !transitioned_to_writing {
                            let _ = writer_phase_tx.send("writing".to_string());
                            transitioned_to_writing = true;
                        }
                        let (queued, errs) = flush_batch(
                            &writer_ctx,
                            library_id,
                            auto_convert,
                            &mut batch,
                            &writer_enrich_tx,
                            &writer_counters,
                        )
                        .await;
                        writer_counters.files_queued.fetch_add(queued, Ordering::Relaxed);
                        writer_counters.errors.fetch_add(errs, Ordering::Relaxed);
                        batch_deadline = None;
                    }
                    if probe_rx.is_closed() {
                        break;
                    }
                }
            }
        }
    });

    // --- Spawn progress emitter ---
    let progress_ctx = ctx.clone();
    let progress_counters = counters.clone();
    let progress_phase_rx = phase_rx.clone();
    let progress_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(PROGRESS_INTERVAL_MS));
        loop {
            interval.tick().await;
            let phase = progress_phase_rx.borrow().clone();
            if phase == "done" {
                break;
            }
            let files_found = progress_counters.files_found.load(Ordering::Relaxed);
            let files_queued = progress_counters.files_queued.load(Ordering::Relaxed);
            let total = progress_counters.total_to_probe.load(Ordering::Relaxed);
            let completed = progress_counters.probes_completed.load(Ordering::Relaxed);
            let to_enrich = progress_counters.items_to_enrich.load(Ordering::Relaxed);
            let enriched = progress_counters.items_enriched.load(Ordering::Relaxed);

            emit_scan_progress(
                &progress_ctx,
                library_id,
                files_found,
                files_queued,
                &phase,
                total,
                completed,
                to_enrich,
                enriched,
            );
        }
    });

    // --- Cascade shutdown via channel drops ---

    // Wait for walk to complete. walk_tx drops → probers drain.
    let _deferred_count = walk_handle.await.unwrap_or(0);
    let _ = phase_tx.send("probing".to_string());

    // Wait for all probers to finish. probe_tx already dropped above → DB writer drains.
    for handle in probe_handles {
        let _ = handle.await;
    }
    let _ = phase_tx.send("writing".to_string());

    // Wait for DB writer to finish.
    let _ = db_writer_handle.await;

    // --- Pass 2: -pb converted files (parallel, bounded by semaphore) ---
    // pb_tx was moved into the walk closure and is already dropped after walk_handle completes.
    let mut pb_files = Vec::new();
    while let Some(path) = pb_rx.recv().await {
        pb_files.push(path);
    }

    if !pb_files.is_empty() && !cancel_token.is_cancelled() {
        let pb_sem = Arc::new(tokio::sync::Semaphore::new(PB_CONCURRENCY));
        let mut pb_handles = Vec::new();

        for pb_path in pb_files {
            let file_path_str = pb_path.to_string_lossy().to_string();
            counters.files_found.fetch_add(1, Ordering::Relaxed);

            // Batch existence check.
            if known_paths.contains(&file_path_str) {
                counters.files_skipped.fetch_add(1, Ordering::Relaxed);
                continue;
            }

            let sem = pb_sem.clone();
            let pb_ctx = ctx.clone();
            let pb_counters = counters.clone();

            let pb_library_id = library_id;
            pb_handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.expect("semaphore closed");
                match ingest_converted_file(&pb_ctx, &pb_path).await {
                    Ok(true) => {
                        tracing::debug!(file = %pb_path.display(), "Scanner linked converted file to item");
                    }
                    Ok(false) => {
                        tracing::warn!(
                            file = %pb_path.display(),
                            "No source item found for converted file, skipping"
                        );
                        pb_counters.files_skipped.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(e) => {
                        let fp = pb_path.to_string_lossy().to_string();
                        tracing::warn!(
                            file = %fp, error = %e,
                            "Failed to ingest converted file"
                        );
                        pb_ctx.event_bus.broadcast(
                            EventCategory::User,
                            EventPayload::LibraryScanError {
                                library_id: pb_library_id,
                                file_path: fp,
                                message: format!("PB ingest failed: {e}"),
                            },
                        );
                        pb_counters.errors.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }));
        }

        for handle in pb_handles {
            let _ = handle.await;
        }
    }

    // --- Scan "complete" — emit completion and release lock ---
    // Enrichment continues in background after this point.

    // Read final counters.
    let files_found = counters.files_found.load(Ordering::Relaxed);
    let files_queued = counters.files_queued.load(Ordering::Relaxed);
    let files_skipped = counters.files_skipped.load(Ordering::Relaxed);
    let errors = counters.errors.load(Ordering::Relaxed);

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
        "Library scan complete (enrichment continues in background)"
    );

    // --- Enrichment phase: wait for all enrichment workers to drain ---
    // This runs after the scan lock is released (ScanGuard drops in the spawner).
    let _ = phase_tx.send("enriching".to_string());
    drop(enrich_tx); // closes channel — workers will drain remaining items then exit

    for handle in enrich_handles {
        let _ = handle.await;
    }

    // --- Stop progress emitter ---
    let _ = phase_tx.send("done".to_string());
    let _ = progress_handle.await;
}

/// Create a pending item during the walk phase.
///
/// For episodes, creates series/season hierarchy first (with ready status).
/// Returns the item_id of the newly created pending item.
fn create_pending_item_for_walk(
    ctx: &AppContext,
    conn: &rusqlite::Connection,
    library_id: sf_core::LibraryId,
    parsed: &sf_parser::ParsedRelease,
    source_file_path: &str,
    series_cache: &mut HashMap<(sf_core::LibraryId, String), sf_db::models::Item>,
    season_cache: &mut HashMap<(sf_core::ItemId, i32), sf_db::models::Item>,
) -> sf_core::Result<sf_core::ItemId> {
    let (item_kind, name, parent_id, season_number, episode_number) =
        if parsed.season.is_some() && parsed.episode.is_some() {
            let season_num = parsed.season.unwrap() as i32;
            let episode_num = parsed.episode.unwrap() as i32;

            // Series cache.
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
                // Emit ItemAdded for newly created series.
                ctx.event_bus.broadcast(
                    EventCategory::User,
                    EventPayload::ItemAdded {
                        item_id: s.id,
                        item_name: s.name.clone(),
                        item_kind: s.item_kind.clone(),
                        library_id,
                    },
                );
                series_cache.insert(cache_key, s.clone());
                s
            };

            // Season cache.
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

            // Build episode name.
            let ep_name = if let Some(end) = parsed.episode_end {
                format!(
                    "{} S{:02}E{:02}E{:02}",
                    parsed.title, season_num, episode_num, end
                )
            } else {
                format!("{} S{:02}E{:02}", parsed.title, season_num, episode_num)
            };

            (
                "episode",
                ep_name,
                Some(season.id),
                Some(season_num),
                Some(episode_num),
            )
        } else {
            (
                "movie",
                parsed.title.clone(),
                None,
                None,
                None,
            )
        };

    let item = sf_db::queries::items::create_pending_item(
        conn,
        library_id,
        item_kind,
        &name,
        parsed.year.map(|y| y as i32),
        parent_id,
        season_number,
        episode_number,
        source_file_path,
    )?;

    // Emit ItemAdded so the frontend can show the item immediately.
    ctx.event_bus.broadcast(
        EventCategory::User,
        EventPayload::ItemAdded {
            item_id: item.id,
            item_name: item.name.clone(),
            item_kind: item.item_kind.clone(),
            library_id,
        },
    );

    Ok(item.id)
}

/// Flush a batch of probe outcomes to the database in a single transaction.
///
/// Creates media_files for successful probes and updates scan_status for all.
/// Returns (files_queued, errors).
async fn flush_batch(
    ctx: &AppContext,
    library_id: sf_core::LibraryId,
    auto_convert: bool,
    batch: &mut Vec<ProbeOutcome>,
    enrich_tx: &tokio::sync::mpsc::Sender<(sf_core::ItemId, sf_core::LibraryId, AppContext)>,
    counters: &ScanCounters,
) -> (u64, u64) {
    let mut files_queued: u64 = 0;
    let mut errors: u64 = 0;

    // Items to enrich after the transaction commits.
    let mut enrich_items: Vec<sf_core::ItemId> = Vec::new();
    // Status changes to emit after commit.
    let mut status_events: Vec<(sf_core::ItemId, String)> = Vec::new();

    {
        let conn = match sf_db::pool::get_conn(&ctx.db) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to get DB connection for batch flush");
                let batch_len = batch.len() as u64;
                for outcome in batch.drain(..) {
                    let (item_id, fp) = match outcome {
                        ProbeOutcome::Success { item_id, walk, .. } => (item_id, walk.file_path_str),
                        ProbeOutcome::Failure { item_id, file_path, .. } => (item_id, file_path),
                    };
                    // Mark as error in DB.
                    if let Ok(conn2) = sf_db::pool::get_conn(&ctx.db) {
                        let _ = sf_db::queries::items::update_item_scan_status(
                            &conn2,
                            item_id,
                            Some("error"),
                            Some(&format!("DB connection failed: {e}")),
                        );
                    }
                    ctx.event_bus.broadcast(
                        EventCategory::User,
                        EventPayload::LibraryScanError {
                            library_id,
                            file_path: fp,
                            message: format!("DB connection failed: {e}"),
                        },
                    );
                }
                return (files_queued, batch_len);
            }
        };

        let tx = match conn.unchecked_transaction() {
            Ok(tx) => tx,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to start transaction for batch flush");
                let batch_len = batch.len() as u64;
                for outcome in batch.drain(..) {
                    let fp = match &outcome {
                        ProbeOutcome::Success { walk, .. } => walk.file_path_str.clone(),
                        ProbeOutcome::Failure { file_path, .. } => file_path.clone(),
                    };
                    ctx.event_bus.broadcast(
                        EventCategory::User,
                        EventPayload::LibraryScanError {
                            library_id,
                            file_path: fp,
                            message: format!("Transaction start failed: {e}"),
                        },
                    );
                }
                return (files_queued, batch_len);
            }
        };

        for outcome in batch.drain(..) {
            match outcome {
                ProbeOutcome::Success {
                    item_id,
                    walk,
                    media_info,
                    file_size,
                } => {
                    let file_path_for_error = walk.file_path_str.clone();
                    match ingest_probed_file(
                        ctx,
                        &tx,
                        library_id,
                        auto_convert,
                        item_id,
                        &walk,
                        &media_info,
                        file_size,
                    ) {
                        Ok(IngestOutcome {
                            queued,
                            enrich_item_id,
                        }) => {
                            // Clear scan_status → ready.
                            let _ = sf_db::queries::items::update_item_scan_status(
                                &tx, item_id, None, None,
                            );
                            status_events.push((item_id, "ready".to_string()));
                            if queued {
                                files_queued += 1;
                            }
                            if let Some(eid) = enrich_item_id {
                                enrich_items.push(eid);
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, file = %file_path_for_error, "Failed to ingest file in batch");
                            let _ = sf_db::queries::items::update_item_scan_status(
                                &tx,
                                item_id,
                                Some("error"),
                                Some(&format!("Ingest failed: {e}")),
                            );
                            status_events.push((item_id, "error".to_string()));
                            ctx.event_bus.broadcast(
                                EventCategory::User,
                                EventPayload::LibraryScanError {
                                    library_id,
                                    file_path: file_path_for_error,
                                    message: format!("Ingest failed: {e}"),
                                },
                            );
                            errors += 1;
                        }
                    }
                }
                ProbeOutcome::Failure {
                    item_id,
                    file_path,
                    error,
                } => {
                    let _ = sf_db::queries::items::update_item_scan_status(
                        &tx,
                        item_id,
                        Some("error"),
                        Some(&error),
                    );
                    status_events.push((item_id, "error".to_string()));
                    ctx.event_bus.broadcast(
                        EventCategory::User,
                        EventPayload::LibraryScanError {
                            library_id,
                            file_path,
                            message: error,
                        },
                    );
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

    // Post-commit: emit status change events.
    for (item_id, scan_status) in status_events {
        ctx.event_bus.broadcast(
            EventCategory::User,
            EventPayload::ItemStatusChanged {
                item_id,
                library_id,
                scan_status,
            },
        );
    }

    // Post-commit: send enrichment requests.
    for eid in enrich_items {
        if enrich_tx.send((eid, library_id, ctx.clone())).await.is_ok() {
            counters.items_to_enrich.fetch_add(1, Ordering::Relaxed);
        }
    }

    (files_queued, errors)
}

struct IngestOutcome {
    queued: bool,
    enrich_item_id: Option<sf_core::ItemId>,
}

/// Ingest a successfully probed file: create media_file + subtitle tracks.
///
/// The item already exists (created during walk). This only creates
/// the media_file record and related data.
fn ingest_probed_file(
    ctx: &AppContext,
    conn: &rusqlite::Connection,
    _library_id: sf_core::LibraryId,
    auto_convert: bool,
    item_id: sf_core::ItemId,
    walk: &WalkResult,
    media_info: &sf_probe::types::MediaInfo,
    file_size: i64,
) -> sf_core::Result<IngestOutcome> {
    let profile = media_info.classify_profile();
    let role = if profile == sf_core::Profile::B {
        "universal"
    } else {
        "source"
    };

    let video = media_info.primary_video();
    let audio = media_info.primary_audio();

    let container = walk.path
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

    // Determine which item to enrich (series for episodes, self for movies).
    let item = sf_db::queries::items::get_item(conn, item_id)?
        .ok_or_else(|| sf_core::Error::not_found("item", item_id))?;

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
        item_id,
        &walk.file_path_str,
        &walk.file_name,
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

    let mut queued = false;

    // Only queue conversion for files that aren't already Profile B.
    if auto_convert && profile != sf_core::Profile::B {
        let job = sf_db::queries::conversion_jobs::create_conversion_job(conn, item_id, mf.id)?;
        ctx.event_bus.broadcast(
            EventCategory::Admin,
            EventPayload::ConversionQueued { job_id: job.id },
        );
        queued = true;
    }

    tracing::debug!(file = %walk.file_path_str, "Scanner ingested file");

    Ok(IngestOutcome {
        queued,
        enrich_item_id,
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

    sf_db::queries::media_files::create_media_file(
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

    Ok(true)
}

/// Best-effort TMDB enrichment for a single item during scan.
///
/// Emits `ItemEnrichmentQueued` before attempting TMDB lookup and
/// `ItemEnriched` when done (success or failure), so the UI can show
/// spinner → sparkle transitions. Early returns (no API key, disabled,
/// already enriched) emit no events — those items never show a spinner.
async fn auto_enrich(ctx: &AppContext, item_id: sf_core::ItemId, library_id: sf_core::LibraryId) {
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

    // Signal to UI that enrichment is starting for this item.
    ctx.event_bus.broadcast(
        EventCategory::User,
        EventPayload::ItemEnrichmentQueued {
            item_id,
            library_id,
        },
    );

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
            // Emit enriched to clear the spinner even on failure.
            ctx.event_bus.broadcast(
                EventCategory::User,
                EventPayload::ItemEnriched { item_id, library_id },
            );
            return;
        }
    };

    let tmdb_id = match results.first() {
        Some(r) => r.id,
        None => {
            // No TMDB results — clear the spinner.
            ctx.event_bus.broadcast(
                EventCategory::User,
                EventPayload::ItemEnriched { item_id, library_id },
            );
            return;
        }
    };

    // Use the enrich_item_with_body helper to do the actual enrichment.
    let _ = crate::routes::metadata::enrich_item_with_body(
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
    .await;

    // Always emit enriched to clear the spinner.
    ctx.event_bus.broadcast(
        EventCategory::User,
        EventPayload::ItemEnriched { item_id, library_id },
    );
}

fn emit_scan_progress(
    ctx: &AppContext,
    library_id: sf_core::LibraryId,
    files_found: u64,
    files_queued: u64,
    phase: &str,
    files_total: u64,
    files_processed: u64,
    items_to_enrich: u64,
    items_enriched: u64,
) {
    ctx.event_bus.broadcast(
        EventCategory::User,
        EventPayload::LibraryScanProgress {
            library_id,
            files_found,
            files_queued,
            phase: phase.to_string(),
            files_total,
            files_processed,
            items_to_enrich,
            items_enriched,
        },
    );
}
