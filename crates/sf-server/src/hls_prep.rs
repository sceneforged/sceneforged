//! Shared helper for populating the in-memory HLS segment cache.

use std::path::Path;
use std::sync::Arc;

use sf_core::{MediaFileId, Result};

use crate::context::AppContext;

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
