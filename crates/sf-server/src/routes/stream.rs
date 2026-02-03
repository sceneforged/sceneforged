//! HLS streaming route handlers.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::context::AppContext;
use crate::error::AppError;

/// GET /api/stream/hls/:item_id/master.m3u8
pub async fn master_playlist(
    State(_ctx): State<AppContext>,
    Path(item_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let _item_id: sf_core::ItemId = item_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item ID".into()))?;

    // In a full implementation, we would look up the media file, generate
    // a segment map, and build the master playlist. For now, return a stub.
    let playlist = sf_media::generate_master_playlist(&sf_media::MasterPlaylist {
        variants: vec![sf_media::Variant {
            bandwidth: 5_000_000,
            resolution: Some((1920, 1080)),
            codecs: "avc1.640028,mp4a.40.2".into(),
            uri: "media.m3u8".into(),
        }],
    });

    Ok((
        StatusCode::OK,
        [("content-type", "application/vnd.apple.mpegurl")],
        playlist,
    ))
}

/// GET /api/stream/hls/:item_id/:segment
pub async fn hls_segment(
    State(_ctx): State<AppContext>,
    Path((item_id, segment)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let _item_id: sf_core::ItemId = item_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item ID".into()))?;

    // In a full implementation, we would read the segment from cache or
    // generate it on-the-fly. For now, return 404.
    Err::<String, _>(sf_core::Error::not_found("segment", segment).into())
}
