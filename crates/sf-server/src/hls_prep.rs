//! Shared helper for populating the in-memory HLS segment cache.

use std::path::Path;
use std::sync::Arc;

use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use sf_core::{MediaFileId, Result};
use sf_media::PreparedMedia;
use tokio::sync::Notify;

use crate::context::AppContext;

/// Maximum number of entries in the HLS cache. When exceeded, excess entries
/// are evicted (arbitrary order). First request after eviction will re-parse
/// the moov atom (~200ms latency).
const MAX_HLS_CACHE_ENTRIES: usize = 200;

/// Drop guard that ensures the `hls_loading` entry is cleaned up even if the
/// loader future is cancelled (e.g. client disconnect). Without this, a stale
/// `Notify` stays in the map and all subsequent requests for the same file
/// deadlock permanently.
struct LoadGuard<'a> {
    loading: &'a DashMap<MediaFileId, Arc<Notify>>,
    key: MediaFileId,
    notify: Arc<Notify>,
}

impl Drop for LoadGuard<'_> {
    fn drop(&mut self) {
        self.loading.remove(&self.key);
        self.notify.notify_waiters();
    }
}

/// Get a `PreparedMedia` from the cache, populating it on demand if missing.
///
/// Uses request coalescing via `ctx.hls_loading` so that concurrent requests
/// for the same file only trigger a single parse.
pub async fn get_or_populate(
    ctx: &AppContext,
    media_file_id: MediaFileId,
) -> Result<Arc<PreparedMedia>> {
    // Fast path: already cached.
    if let Some(entry) = ctx.hls_cache.get(&media_file_id) {
        return Ok(entry.value().clone());
    }

    // Slow path: need to populate. Use coalescing to avoid duplicate work.
    loop {
        match ctx.hls_loading.entry(media_file_id) {
            Entry::Occupied(e) => {
                // Another task is already loading this file — wait for it.
                let notify = e.get().clone();
                drop(e);
                notify.notified().await;

                // Re-check cache: the loader should have populated it.
                if let Some(entry) = ctx.hls_cache.get(&media_file_id) {
                    return Ok(entry.value().clone());
                }
                // Loader failed — loop to try becoming the loader ourselves.
            }
            Entry::Vacant(e) => {
                // We're the loader. Insert our Notify so others can wait.
                let notify = Arc::new(Notify::new());
                e.insert(notify.clone());

                // Guard ensures cleanup even if this future is cancelled.
                let guard = LoadGuard {
                    loading: &ctx.hls_loading,
                    key: media_file_id,
                    notify,
                };

                let result = do_populate(ctx, media_file_id).await;

                // Explicit drop — guard's Drop removes the loading entry and
                // wakes all waiters regardless of success/failure/cancellation.
                drop(guard);

                return result;
            }
        }
    }
}

/// Look up the media file in DB, parse its moov, build PreparedMedia, and
/// insert into the cache.
async fn do_populate(
    ctx: &AppContext,
    media_file_id: MediaFileId,
) -> Result<Arc<PreparedMedia>> {
    // Look up the media file to get its path.
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let mf = sf_db::queries::media_files::get_media_file(&conn, media_file_id)?
        .ok_or_else(|| sf_core::Error::not_found("media_file", media_file_id))?;
    drop(conn);

    let path = std::path::PathBuf::from(&mf.file_path);
    if !path.exists() {
        return Err(sf_core::Error::not_found("file", &mf.file_path));
    }

    // Parse and build (CPU-bound work on blocking thread).
    populate_hls_cache(ctx, media_file_id, &path).await?;

    ctx.hls_cache
        .get(&media_file_id)
        .map(|entry| entry.value().clone())
        .ok_or_else(|| {
            sf_core::Error::Internal("HLS cache entry missing after populate".into())
        })
}

/// Parse the moov atom from a Profile B MP4 and insert precomputed segment
/// data into the in-memory HLS cache.
pub async fn populate_hls_cache(
    ctx: &AppContext,
    media_file_id: MediaFileId,
    path: &Path,
) -> Result<()> {
    let path = path.to_path_buf();

    let prepared = tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path).map_err(|e| {
            sf_core::Error::Internal(format!("Failed to open {}: {e}", path.display()))
        })?;
        let mut reader = std::io::BufReader::new(file);

        let metadata = sf_media::parse_moov(&mut reader).map_err(|e| {
            sf_core::Error::Internal(format!("Failed to parse moov in {}: {e}", path.display()))
        })?;

        sf_media::build_prepared_media(&metadata, &path).map_err(|e| {
            sf_core::Error::Internal(format!(
                "Failed to build prepared media for {}: {e}",
                path.display()
            ))
        })
    })
    .await
    .map_err(|e| sf_core::Error::Internal(format!("spawn_blocking join error: {e}")))??;

    ctx.hls_cache.insert(media_file_id, Arc::new(prepared));
    tracing::debug!(media_file_id = %media_file_id, "HLS cache populated");

    // Evict excess entries to keep memory bounded.
    evict_if_over_limit(&ctx.hls_cache);

    Ok(())
}

/// Evict entries from the HLS cache if it exceeds `MAX_HLS_CACHE_ENTRIES`.
///
/// Evicts in arbitrary order (DashMap iteration order). This is intentionally
/// simple — more sophisticated LRU tracking isn't worth the complexity for a
/// cache that repopulates on demand in ~200ms.
fn evict_if_over_limit(
    cache: &dashmap::DashMap<MediaFileId, Arc<PreparedMedia>>,
) {
    let len = cache.len();
    if len <= MAX_HLS_CACHE_ENTRIES {
        return;
    }

    let to_remove = len - MAX_HLS_CACHE_ENTRIES;
    let keys: Vec<MediaFileId> = cache
        .iter()
        .take(to_remove)
        .map(|entry| *entry.key())
        .collect();

    for key in &keys {
        cache.remove(key);
    }

    tracing::debug!(
        evicted = keys.len(),
        remaining = cache.len(),
        "HLS cache eviction"
    );
}
