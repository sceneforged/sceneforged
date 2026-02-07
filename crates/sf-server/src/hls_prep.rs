//! Shared helper for populating the in-memory HLS segment cache.

use std::path::Path;
use std::sync::Arc;

use dashmap::mapref::entry::Entry;
use sf_core::{MediaFileId, Result};
use sf_media::PreparedMedia;
use tokio::sync::Notify;

use crate::context::AppContext;

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

                // Do the actual work, cleaning up on both success and failure.
                let result = do_populate(ctx, media_file_id).await;

                // Remove our loading entry and notify all waiters.
                ctx.hls_loading.remove(&media_file_id);
                notify.notify_waiters();

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

    Ok(())
}

/// Warm the HLS cache at startup by loading all Profile B media files.
pub async fn warm_hls_cache(ctx: &AppContext) -> Result<()> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let media_files = sf_db::queries::media_files::list_media_files_by_profile(&conn, "B")?;
    drop(conn);

    let count = media_files.len();
    if count == 0 {
        return Ok(());
    }

    tracing::info!("Warming HLS cache for {count} Profile B media files");

    let mut loaded = 0u64;
    let mut errors = 0u64;

    for mf in media_files {
        let path = Path::new(&mf.file_path);
        if !path.exists() {
            tracing::debug!(
                media_file_id = %mf.id,
                path = %mf.file_path,
                "Skipping HLS warmup: file not found"
            );
            continue;
        }

        match populate_hls_cache(ctx, mf.id, path).await {
            Ok(()) => loaded += 1,
            Err(e) => {
                tracing::warn!(
                    media_file_id = %mf.id,
                    error = %e,
                    "Failed to warm HLS cache entry"
                );
                errors += 1;
            }
        }
    }

    tracing::info!("HLS cache warmup complete: {loaded} loaded, {errors} errors");
    Ok(())
}
