//! Streaming route handlers: HLS (from precomputed segment cache) and direct
//! file streaming.
//!
//! HLS segments are served zero-copy: moof+mdat headers come from RAM, sample
//! data is read from the source MP4 file on demand.
//! Direct streaming serves source files with HTTP range request support.

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;

use crate::context::AppContext;
use crate::error::AppError;
use crate::hls_prep;

/// GET /api/stream/:media_file_id/index.m3u8
pub async fn hls_playlist(
    State(ctx): State<AppContext>,
    Path(media_file_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let mf_id: sf_core::MediaFileId = media_file_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid media_file_id".into()))?;

    let prepared = hls_prep::get_or_populate(&ctx, mf_id).await?;

    Ok((
        StatusCode::OK,
        [("content-type", "application/vnd.apple.mpegurl")],
        prepared.variant_playlist.clone(),
    ))
}

/// GET /api/stream/:media_file_id/:segment
///
/// Serves `init.mp4` or `segment_N.m4s` from the in-memory cache + source file.
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

    let prepared = hls_prep::get_or_populate(&ctx, mf_id).await?;

    if segment == "init.mp4" {
        return Ok((
            StatusCode::OK,
            [("content-type", "video/mp4")],
            prepared.init_segment.clone(),
        )
            .into_response());
    }

    // Parse segment_N.m4s
    let seg_index = segment
        .strip_prefix("segment_")
        .and_then(|s| s.strip_suffix(".m4s"))
        .and_then(|s| s.parse::<usize>().ok())
        .ok_or_else(|| sf_core::Error::not_found("segment", &segment))?;

    let seg = prepared
        .segments
        .get(seg_index)
        .ok_or_else(|| sf_core::Error::not_found("segment", &segment))?;

    // Assemble the segment: moof_bytes + mdat_header + data from source file.
    let total_size =
        seg.moof_bytes.len() + seg.mdat_header.len() + seg.data_length as usize;
    let mut buf = Vec::with_capacity(total_size);
    buf.extend_from_slice(&seg.moof_bytes);
    buf.extend_from_slice(&seg.mdat_header);

    // Read data ranges from source file: video first, then audio, to match
    // the trun data_offset layout in the moof.
    let file_path = prepared.file_path.clone();
    let video_ranges: Vec<(u64, u64)> = seg
        .video_data_ranges
        .iter()
        .map(|r| (r.file_offset, r.length))
        .collect();
    let audio_ranges: Vec<(u64, u64)> = seg
        .audio_data_ranges
        .iter()
        .map(|r| (r.file_offset, r.length))
        .collect();
    let expected_data = seg.data_length as usize;

    let data = tokio::task::spawn_blocking(move || -> sf_core::Result<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        let mut file = std::fs::File::open(&file_path).map_err(|e| {
            sf_core::Error::Internal(format!(
                "Failed to open {}: {e}",
                file_path.display()
            ))
        })?;
        let mut data = Vec::with_capacity(expected_data);
        // Video data first (matches video trun data_offset).
        for (offset, length) in &video_ranges {
            file.seek(SeekFrom::Start(*offset)).map_err(|e| {
                sf_core::Error::Internal(format!("Seek failed: {e}"))
            })?;
            let mut chunk = vec![0u8; *length as usize];
            file.read_exact(&mut chunk).map_err(|e| {
                sf_core::Error::Internal(format!("Read failed: {e}"))
            })?;
            data.extend_from_slice(&chunk);
        }
        // Audio data second (matches audio trun data_offset).
        for (offset, length) in &audio_ranges {
            file.seek(SeekFrom::Start(*offset)).map_err(|e| {
                sf_core::Error::Internal(format!("Seek failed: {e}"))
            })?;
            let mut chunk = vec![0u8; *length as usize];
            file.read_exact(&mut chunk).map_err(|e| {
                sf_core::Error::Internal(format!("Read failed: {e}"))
            })?;
            data.extend_from_slice(&chunk);
        }
        Ok(data)
    })
    .await
    .map_err(|e| sf_core::Error::Internal(format!("spawn_blocking join error: {e}")))??;

    buf.extend_from_slice(&data);

    Ok((
        StatusCode::OK,
        [("content-type", "video/iso.segment")],
        buf,
    )
        .into_response())
}

/// GET /api/stream/:media_file_id/direct
///
/// Serve the source file directly with HTTP range request support.
/// Uses chunked streaming via `ReaderStream` to avoid loading entire files
/// into memory. Sendfile(2) intercepts most direct play requests before they
/// reach this handler; this is the safety-net fallback.
pub async fn direct_stream(
    State(ctx): State<AppContext>,
    Path(media_file_id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let mf_id: sf_core::MediaFileId = media_file_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid media_file_id".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let mf = sf_db::queries::media_files::get_media_file(&conn, mf_id)?
        .ok_or_else(|| sf_core::Error::not_found("media_file", mf_id))?;

    let file_path = std::path::Path::new(&mf.file_path);
    let range_header = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned());

    Ok(super::streaming_helpers::serve_file_streaming(
        file_path,
        &mf.file_name,
        mf.container.as_deref(),
        range_header.as_deref(),
    )
    .await?)
}
