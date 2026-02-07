//! Subtitle track listing and extraction routes.

use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;

use crate::context::AppContext;
use crate::error::AppError;

#[derive(Debug, Serialize)]
pub struct SubtitleTrackResponse {
    pub id: String,
    pub media_file_id: String,
    pub track_index: i32,
    pub codec: String,
    pub language: Option<String>,
    pub forced: bool,
    pub default_track: bool,
}

/// GET /api/items/{id}/subtitles — list subtitle tracks for an item.
pub async fn list_subtitles(
    State(ctx): State<AppContext>,
    Path(item_id): Path<String>,
) -> Result<Json<Vec<SubtitleTrackResponse>>, AppError> {
    let id: sf_core::ItemId = item_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item_id".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;

    // Get all media files for this item, then collect subtitle tracks.
    let media_files = sf_db::queries::media_files::list_media_files_by_item(&conn, id)?;
    let mut tracks = Vec::new();

    for mf in &media_files {
        let mf_tracks = sf_db::queries::subtitle_tracks::list_by_media_file(&conn, mf.id)?;
        for t in mf_tracks {
            tracks.push(SubtitleTrackResponse {
                id: t.id.to_string(),
                media_file_id: t.media_file_id.to_string(),
                track_index: t.track_index,
                codec: t.codec,
                language: t.language,
                forced: t.forced,
                default_track: t.default_track,
            });
        }
    }

    Ok(Json(tracks))
}

/// GET /api/stream/{media_file_id}/subtitles/{track_index}
///
/// Extract a subtitle track from the source file via ffmpeg, serving as
/// WebVTT for browser compatibility.
pub async fn get_subtitle(
    State(ctx): State<AppContext>,
    Path((media_file_id, track_index)): Path<(String, i32)>,
) -> Result<impl IntoResponse, AppError> {
    let mf_id: sf_core::MediaFileId = media_file_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid media_file_id".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let mf = sf_db::queries::media_files::get_media_file(&conn, mf_id)?
        .ok_or_else(|| sf_core::Error::not_found("media_file", mf_id))?;

    // Verify track exists.
    let tracks = sf_db::queries::subtitle_tracks::list_by_media_file(&conn, mf_id)?;
    let track = tracks
        .iter()
        .find(|t| t.track_index == track_index)
        .ok_or_else(|| {
            sf_core::Error::not_found("subtitle_track", format!("{mf_id}:{track_index}"))
        })?;
    drop(conn);

    // Extract via ffmpeg to WebVTT.
    let output = tokio::process::Command::new("ffmpeg")
        .args([
            "-i",
            &mf.file_path,
            "-map",
            &format!("0:s:{}", track.track_index),
            "-f",
            "webvtt",
            "-",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .await
        .map_err(|e| sf_core::Error::Internal(format!("ffmpeg subtitle extraction failed: {e}")))?;

    if !output.status.success() {
        return Err(
            sf_core::Error::Internal("ffmpeg subtitle extraction returned non-zero".into()).into(),
        );
    }

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/vtt; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        output.stdout,
    ))
}
