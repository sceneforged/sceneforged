//! Shared helper for populating the in-memory HLS segment cache.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use dashmap::mapref::entry::Entry;
use sf_core::{MediaFileId, Result};
use sf_media::PreparedMedia;
use tokio::sync::Notify;

use crate::context::AppContext;

/// Maximum number of entries in the HLS cache. When exceeded, excess entries
/// are evicted (arbitrary order). First request after eviction will re-parse
/// the moov atom (~200ms latency).
const MAX_HLS_CACHE_ENTRIES: usize = 200;

/// Maximum attempts before giving up (prevents infinite loops on corrupt files).
const MAX_POPULATE_ATTEMPTS: usize = 2;

/// Get a `PreparedMedia` from the cache, populating it on demand if missing.
///
/// Uses request coalescing via `ctx.hls_loading` so that concurrent requests
/// for the same file only trigger a single parse. Population runs in a detached
/// `tokio::spawn` task so it survives client disconnects.
pub async fn get_or_populate(
    ctx: &AppContext,
    media_file_id: MediaFileId,
) -> Result<Arc<PreparedMedia>> {
    let mut attempts = 0;

    loop {
        // Check cache at the top of every iteration — handles the race where
        // population completes between our Vacant insert and the next loop.
        if let Some(mut entry) = ctx.hls_cache.get_mut(&media_file_id) {
            entry.1 = Instant::now();
            return Ok(entry.0.clone());
        }

        if attempts >= MAX_POPULATE_ATTEMPTS {
            return Err(sf_core::Error::Internal(format!(
                "HLS cache population failed after {MAX_POPULATE_ATTEMPTS} attempts for {media_file_id}"
            )));
        }

        match ctx.hls_loading.entry(media_file_id) {
            Entry::Occupied(e) => {
                // Another task is already loading this file — wait for it.
                let notify = e.get().clone();
                drop(e);
                notify.notified().await;
                attempts += 1;
                // Loop back to check cache.
            }
            Entry::Vacant(e) => {
                // We're the first — insert Notify and spawn a detached task.
                let notify = Arc::new(Notify::new());
                e.insert(notify.clone());

                // Use a oneshot channel so the spawner gets the actual error back.
                let (tx, rx) = tokio::sync::oneshot::channel::<Result<Arc<PreparedMedia>>>();

                let ctx2 = ctx.clone();
                tokio::spawn(async move {
                    let result = do_populate(&ctx2, media_file_id).await;
                    if let Err(ref e) = result {
                        tracing::warn!(media_file_id = %media_file_id, error = %e, "HLS populate failed");
                    }
                    // Always clean up and wake waiters, even on failure.
                    ctx2.hls_loading.remove(&media_file_id);
                    notify.notify_waiters();
                    // Send result to the original caller (ignore error if they disconnected).
                    let _ = tx.send(result);
                });

                // Wait for the detached task to finish and get the result.
                match rx.await {
                    Ok(result) => return result,
                    Err(_) => {
                        // Task panicked — loop to retry.
                        attempts += 1;
                    }
                }
            }
        }
    }
}

/// Look up the media file in DB, parse its moov, build PreparedMedia, and
/// insert into the cache. All blocking work runs inside `spawn_blocking`
/// so it never blocks the async runtime AND survives cancellation.
async fn do_populate(
    ctx: &AppContext,
    media_file_id: MediaFileId,
) -> Result<Arc<PreparedMedia>> {
    let db = ctx.db.clone();
    let hls_cache = ctx.hls_cache.clone();

    let prepared = tokio::task::spawn_blocking(move || {
        tracing::debug!(media_file_id = %media_file_id, "do_populate: acquiring DB connection");
        let conn = sf_db::pool::get_conn(&db)?;
        tracing::debug!(media_file_id = %media_file_id, "do_populate: DB connection acquired");

        let mf = sf_db::queries::media_files::get_media_file(&conn, media_file_id)?
            .ok_or_else(|| sf_core::Error::not_found("media_file", media_file_id))?;
        drop(conn);
        tracing::debug!(media_file_id = %media_file_id, path = %mf.file_path, "do_populate: media file found");

        let path = std::path::PathBuf::from(&mf.file_path);
        if !path.exists() {
            return Err(sf_core::Error::not_found("file", &mf.file_path));
        }

        let file = std::fs::File::open(&path).map_err(|e| {
            sf_core::Error::Internal(format!("Failed to open {}: {e}", path.display()))
        })?;
        let mut reader = std::io::BufReader::new(file);

        tracing::debug!(media_file_id = %media_file_id, "do_populate: parsing moov atom");
        let metadata = sf_media::parse_moov(&mut reader).map_err(|e| {
            sf_core::Error::Internal(format!("Failed to parse moov in {}: {e}", path.display()))
        })?;

        tracing::debug!(media_file_id = %media_file_id, "do_populate: building prepared media");
        let prepared = sf_media::build_prepared_media(&metadata, &path).map_err(|e| {
            sf_core::Error::Internal(format!(
                "Failed to build prepared media for {}: {e}",
                path.display()
            ))
        })?;

        let prepared = Arc::new(prepared);

        // Insert into cache INSIDE spawn_blocking — non-cancellable.
        hls_cache.insert(media_file_id, (prepared.clone(), Instant::now()));
        tracing::debug!(media_file_id = %media_file_id, "do_populate: HLS cache populated");

        // Evict excess entries.
        evict_if_over_limit(&hls_cache);

        Ok(prepared)
    })
    .await
    .map_err(|e| sf_core::Error::Internal(format!("spawn_blocking join error: {e}")))??;

    Ok(prepared)
}

/// Parse the moov atom from a Profile B MP4 and insert precomputed segment
/// data into the in-memory HLS cache.
pub async fn populate_hls_cache(
    ctx: &AppContext,
    media_file_id: MediaFileId,
    path: &Path,
) -> Result<()> {
    let path = path.to_path_buf();
    let hls_cache = ctx.hls_cache.clone();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path).map_err(|e| {
            sf_core::Error::Internal(format!("Failed to open {}: {e}", path.display()))
        })?;
        let mut reader = std::io::BufReader::new(file);

        let metadata = sf_media::parse_moov(&mut reader).map_err(|e| {
            sf_core::Error::Internal(format!("Failed to parse moov in {}: {e}", path.display()))
        })?;

        let prepared = sf_media::build_prepared_media(&metadata, &path).map_err(|e| {
            sf_core::Error::Internal(format!(
                "Failed to build prepared media for {}: {e}",
                path.display()
            ))
        })?;

        // Insert into cache inside spawn_blocking — non-cancellable.
        hls_cache.insert(media_file_id, (Arc::new(prepared), Instant::now()));
        tracing::debug!(media_file_id = %media_file_id, "HLS cache populated");

        // Evict excess entries.
        evict_if_over_limit(&hls_cache);

        Ok(())
    })
    .await
    .map_err(|e| sf_core::Error::Internal(format!("spawn_blocking join error: {e}")))?
}

/// Evict the least-recently-used entries from the HLS cache if it exceeds
/// `MAX_HLS_CACHE_ENTRIES`.
///
/// Collects all `(key, last_access)` pairs, sorts by timestamp, and removes
/// the oldest entries. This avoids evicting actively-streamed files that
/// would cause a ~200ms re-parse stutter.
fn evict_if_over_limit(
    cache: &dashmap::DashMap<MediaFileId, (Arc<PreparedMedia>, Instant)>,
) {
    let len = cache.len();
    if len <= MAX_HLS_CACHE_ENTRIES {
        return;
    }

    let to_remove = len - MAX_HLS_CACHE_ENTRIES;
    let mut entries: Vec<(MediaFileId, Instant)> = cache
        .iter()
        .map(|e| (*e.key(), e.value().1))
        .collect();
    entries.sort_by_key(|&(_, ts)| ts);

    let keys: Vec<MediaFileId> = entries.into_iter().take(to_remove).map(|(k, _)| k).collect();
    for key in &keys {
        cache.remove(key);
    }

    tracing::debug!(evicted = keys.len(), remaining = cache.len(), "HLS cache LRU eviction");
}
