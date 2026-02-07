//! Jellyfin-compatible streaming and playback info endpoints.

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::context::AppContext;
use crate::error::AppError;
use crate::hls_prep;

use super::dto::{MediaSourceDto, MediaStreamDto, TICKS_PER_SECOND};

/// Jellyfin PlaybackInfo response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlaybackInfoResponse {
    pub media_sources: Vec<MediaSourceDto>,
    pub play_session_id: String,
}

/// POST /Items/{id}/PlaybackInfo — return media sources for a playable item.
pub async fn playback_info(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> Result<Json<PlaybackInfoResponse>, AppError> {
    let item_id: sf_core::ItemId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item_id".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    // Verify item exists.
    sf_db::queries::items::get_item(&conn, item_id)?
        .ok_or_else(|| sf_core::Error::not_found("item", item_id))?;

    let media_files = sf_db::queries::media_files::list_media_files_by_item(&conn, item_id)?;

    let sources: Vec<MediaSourceDto> = media_files
        .iter()
        .map(|mf| {
            let ticks = mf
                .duration_secs
                .map(|d| (d * TICKS_PER_SECOND as f64) as i64);

            // Build media streams from the flat fields on MediaFile.
            let mut streams = Vec::new();
            let mut idx = 0i32;

            // Video stream (if codec info exists).
            if let Some(ref codec) = mf.video_codec {
                let display = match (mf.resolution_width, mf.resolution_height) {
                    (Some(w), Some(h)) => format!("{w}x{h} {codec}"),
                    _ => codec.clone(),
                };
                streams.push(MediaStreamDto {
                    stream_type: "Video".to_string(),
                    index: idx,
                    codec: Some(codec.clone()),
                    language: None,
                    display_title: Some(display),
                    is_default: true,
                    is_forced: false,
                    width: mf.resolution_width,
                    height: mf.resolution_height,
                });
                idx += 1;
            }

            // Audio stream (if codec info exists).
            if let Some(ref codec) = mf.audio_codec {
                streams.push(MediaStreamDto {
                    stream_type: "Audio".to_string(),
                    index: idx,
                    codec: Some(codec.clone()),
                    language: None,
                    display_title: Some(codec.clone()),
                    is_default: true,
                    is_forced: false,
                    width: None,
                    height: None,
                });
            }

            MediaSourceDto {
                id: mf.id.to_string(),
                name: mf.file_name.clone(),
                path: mf.file_path.clone(),
                container: mf.container.clone(),
                size: Some(mf.file_size),
                run_time_ticks: ticks,
                supports_direct_stream: true,
                supports_direct_play: true,
                supports_transcoding: false,
                media_streams: if streams.is_empty() {
                    None
                } else {
                    Some(streams)
                },
            }
        })
        .collect();

    Ok(Json(PlaybackInfoResponse {
        media_sources: sources,
        play_session_id: uuid::Uuid::new_v4().to_string(),
    }))
}

#[derive(Debug, Deserialize)]
pub struct VideoStreamQuery {
    #[serde(alias = "mediaSourceId", alias = "MediaSourceId")]
    pub media_source_id: Option<String>,
    #[serde(alias = "static", alias = "Static")]
    pub is_static: Option<bool>,
}

/// GET /Videos/{id}/stream — direct file streaming (with range support).
///
/// Jellyfin clients request this for direct play. The `id` is the item ID;
/// `mediaSourceId` query param selects which media file.
pub async fn video_stream(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
    Query(params): Query<VideoStreamQuery>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let item_id: sf_core::ItemId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item_id".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;

    // Find the media file — either by explicit mediaSourceId or first file for item.
    let mf = if let Some(ref ms_id) = params.media_source_id {
        let mf_id: sf_core::MediaFileId = ms_id
            .parse()
            .map_err(|_| sf_core::Error::Validation("Invalid mediaSourceId".into()))?;
        sf_db::queries::media_files::get_media_file(&conn, mf_id)?
            .ok_or_else(|| sf_core::Error::not_found("media_file", ms_id.as_str()))?
    } else {
        let files = sf_db::queries::media_files::list_media_files_by_item(&conn, item_id)?;
        files
            .into_iter()
            .next()
            .ok_or_else(|| sf_core::Error::not_found("media_file for item", item_id))?
    };

    let file_path = std::path::PathBuf::from(&mf.file_path);
    let metadata = tokio::fs::metadata(&file_path)
        .await
        .map_err(|_| sf_core::Error::not_found("file", &mf.file_path))?;

    let file_size = metadata.len();
    let content_type = guess_content_type(&mf.file_name, mf.container.as_deref());

    // Parse Range header.
    let range = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_range_header);

    match range {
        Some((start, end_opt)) => {
            let end = end_opt.unwrap_or(file_size - 1).min(file_size - 1);
            if start > end || start >= file_size {
                return Ok((
                    StatusCode::RANGE_NOT_SATISFIABLE,
                    [(header::CONTENT_RANGE.as_str(), format!("bytes */{file_size}"))],
                    Vec::new(),
                )
                    .into_response());
            }

            let length = end - start + 1;
            let mut file = tokio::fs::File::open(&file_path).await.map_err(|_| {
                sf_core::Error::not_found("file", &mf.file_path)
            })?;
            tokio::io::AsyncSeekExt::seek(&mut file, std::io::SeekFrom::Start(start))
                .await
                .map_err(|e| sf_core::Error::Internal(format!("Seek failed: {e}")))?;

            let mut buf = vec![0u8; length as usize];
            tokio::io::AsyncReadExt::read_exact(&mut file, &mut buf)
                .await
                .map_err(|e| sf_core::Error::Internal(format!("Read failed: {e}")))?;

            Ok((
                StatusCode::PARTIAL_CONTENT,
                [
                    (header::CONTENT_TYPE.as_str(), content_type.to_string()),
                    (
                        header::CONTENT_RANGE.as_str(),
                        format!("bytes {start}-{end}/{file_size}"),
                    ),
                    (header::CONTENT_LENGTH.as_str(), length.to_string()),
                    (header::ACCEPT_RANGES.as_str(), "bytes".to_string()),
                ],
                buf,
            )
                .into_response())
        }
        None => {
            let data = tokio::fs::read(&file_path).await.map_err(|_| {
                sf_core::Error::not_found("file", &mf.file_path)
            })?;

            Ok((
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE.as_str(), content_type.to_string()),
                    (header::CONTENT_LENGTH.as_str(), file_size.to_string()),
                    (header::ACCEPT_RANGES.as_str(), "bytes".to_string()),
                ],
                data,
            )
                .into_response())
        }
    }
}

/// GET /Videos/{id}/master.m3u8 — HLS master playlist.
///
/// Jellyfin clients request this for HLS playback. Redirects to our internal
/// HLS endpoint if a Profile B conversion exists.
pub async fn master_playlist(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
    Query(params): Query<VideoStreamQuery>,
) -> Result<impl IntoResponse, AppError> {
    let item_id: sf_core::ItemId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item_id".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;

    // Find a Profile B media file for HLS.
    let media_files = sf_db::queries::media_files::list_media_files_by_item(&conn, item_id)?;
    let profile_b = media_files.iter().find(|mf| mf.profile == "B");

    let mf = if let Some(pb) = profile_b {
        pb
    } else if let Some(ref ms_id) = params.media_source_id {
        let mf_id: sf_core::MediaFileId = ms_id
            .parse()
            .map_err(|_| sf_core::Error::Validation("Invalid mediaSourceId".into()))?;
        media_files
            .iter()
            .find(|mf| mf.id == mf_id)
            .ok_or_else(|| sf_core::Error::not_found("media_file", ms_id.as_str()))?
    } else {
        media_files
            .first()
            .ok_or_else(|| sf_core::Error::not_found("media_file for item", item_id))?
    };

    // Get the HLS playlist from our cache.
    let prepared = hls_prep::get_or_populate(&ctx, mf.id).await?;

    Ok((
        StatusCode::OK,
        [("content-type", "application/vnd.apple.mpegurl")],
        prepared.variant_playlist.clone(),
    ))
}

// -- Helpers (duplicated from routes/stream.rs to avoid coupling) --

fn parse_range_header(value: &str) -> Option<(u64, Option<u64>)> {
    let bytes_prefix = value.strip_prefix("bytes=")?;
    let mut parts = bytes_prefix.splitn(2, '-');
    let start_str = parts.next()?.trim();
    let end_str = parts.next()?.trim();

    let start: u64 = start_str.parse().ok()?;
    let end: Option<u64> = if end_str.is_empty() {
        None
    } else {
        Some(end_str.parse().ok()?)
    };

    Some((start, end))
}

fn guess_content_type(file_name: &str, container: Option<&str>) -> &'static str {
    let ext = container
        .or_else(|| file_name.rsplit('.').next())
        .unwrap_or("");

    match ext {
        "mp4" | "m4v" => "video/mp4",
        "mkv" => "video/x-matroska",
        "avi" => "video/x-msvideo",
        "webm" => "video/webm",
        "ts" => "video/mp2t",
        "mov" => "video/quicktime",
        _ => "application/octet-stream",
    }
}
