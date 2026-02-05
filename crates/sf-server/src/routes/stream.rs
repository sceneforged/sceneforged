//! HLS streaming route handlers.
//!
//! Serves pre-generated HLS fMP4 segments from disk, keyed by media_file_id.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::context::AppContext;
use crate::error::AppError;

/// GET /api/stream/:media_file_id/index.m3u8
pub async fn hls_playlist(
    State(ctx): State<AppContext>,
    Path(media_file_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let mf_id: sf_core::MediaFileId = media_file_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid media_file_id".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let cache = sf_db::queries::hls_cache::get_hls_cache(&conn, mf_id)?
        .ok_or_else(|| sf_core::Error::not_found("hls_cache", mf_id))?;

    let playlist_path = std::path::Path::new(&cache.playlist).join("index.m3u8");
    let content = tokio::fs::read_to_string(&playlist_path)
        .await
        .map_err(|_| {
            sf_core::Error::not_found("hls_playlist", playlist_path.to_string_lossy())
        })?;

    Ok((
        StatusCode::OK,
        [("content-type", "application/vnd.apple.mpegurl")],
        content,
    ))
}

/// GET /api/stream/:media_file_id/:segment
///
/// Serves `init.mp4` or `seg*.m4s` files from the HLS cache directory.
pub async fn hls_segment(
    State(ctx): State<AppContext>,
    Path((media_file_id, segment)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let mf_id: sf_core::MediaFileId = media_file_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid media_file_id".into()))?;

    // Validate segment filename to prevent directory traversal.
    if segment.contains('/')
        || segment.contains('\\')
        || segment.contains("..")
        || segment.starts_with('.')
    {
        return Err(sf_core::Error::Validation("Invalid segment filename".into()).into());
    }

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let cache = sf_db::queries::hls_cache::get_hls_cache(&conn, mf_id)?
        .ok_or_else(|| sf_core::Error::not_found("hls_cache", mf_id))?;

    let segment_path = std::path::Path::new(&cache.playlist).join(&segment);
    let data = tokio::fs::read(&segment_path)
        .await
        .map_err(|_| sf_core::Error::not_found("segment", &segment))?;

    let content_type = if segment.ends_with(".m4s") {
        "video/iso.segment"
    } else if segment.ends_with(".mp4") {
        "video/mp4"
    } else {
        "application/octet-stream"
    };

    Ok((
        StatusCode::OK,
        [("content-type", content_type)],
        data,
    ))
}
