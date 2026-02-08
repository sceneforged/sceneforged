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
                idx += 1;
            }

            // Subtitle streams from subtitle_tracks table.
            if let Ok(subtitle_tracks) = sf_db::queries::subtitle_tracks::list_by_media_file(&conn, mf.id) {
                for track in &subtitle_tracks {
                    let display = track.language.as_deref().unwrap_or("Unknown");
                    let mut title = display.to_string();
                    if track.forced {
                        title.push_str(" (Forced)");
                    }
                    streams.push(MediaStreamDto {
                        stream_type: "Subtitle".to_string(),
                        index: idx,
                        codec: Some(track.codec.clone()),
                        language: track.language.clone(),
                        display_title: Some(title),
                        is_default: track.default_track,
                        is_forced: track.forced,
                        width: None,
                        height: None,
                    });
                    idx += 1;
                }
            }

            let direct_stream_url = format!(
                "/Videos/{}/stream?mediaSourceId={}&static=true",
                item_id, mf.id,
            );
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
                protocol: "File".to_string(),
                media_source_type: "Default".to_string(),
                direct_stream_url: Some(direct_stream_url),
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
/// Uses chunked streaming via `ReaderStream` to avoid loading entire files
/// into memory. Sendfile(2) intercepts most requests before they reach this
/// handler; this is the safety-net fallback.
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
    let range_header = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned());

    Ok(crate::routes::streaming_helpers::serve_file_streaming(
        &file_path,
        &mf.file_name,
        mf.container.as_deref(),
        range_header.as_deref(),
    )
    .await?)
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

