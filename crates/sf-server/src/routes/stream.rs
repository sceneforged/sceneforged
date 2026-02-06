//! Streaming route handlers: HLS (from precomputed segment cache) and direct
//! file streaming.
//!
//! HLS segments are served zero-copy: moof+mdat headers come from RAM, sample
//! data is read from the source MP4 file on demand.
//! Direct streaming serves source files with HTTP range request support.

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use tokio::io::AsyncSeekExt;

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

    let prepared = ctx
        .hls_cache
        .get(&mf_id)
        .map(|entry| entry.value().clone())
        .ok_or_else(|| sf_core::Error::not_found("hls_cache", mf_id))?;

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

    let prepared = ctx
        .hls_cache
        .get(&mf_id)
        .map(|entry| entry.value().clone())
        .ok_or_else(|| sf_core::Error::not_found("hls_cache", mf_id))?;

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
/// Supports `Range: bytes=START-END` headers for seeking.
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
    let metadata = tokio::fs::metadata(file_path)
        .await
        .map_err(|_| sf_core::Error::not_found("file", &mf.file_path))?;

    let file_size = metadata.len();
    let content_type = guess_content_type(&mf.file_name, mf.container.as_deref());

    // Parse Range header
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
                    [
                        (
                            header::CONTENT_RANGE.as_str(),
                            format!("bytes */{file_size}"),
                        ),
                    ],
                    Vec::new(),
                )
                    .into_response());
            }

            let length = end - start + 1;

            let mut file = tokio::fs::File::open(file_path).await.map_err(|_| {
                sf_core::Error::not_found("file", &mf.file_path)
            })?;
            file.seek(std::io::SeekFrom::Start(start)).await.map_err(|e| {
                sf_core::Error::Internal(format!("Seek failed: {e}"))
            })?;

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
            let data = tokio::fs::read(file_path).await.map_err(|_| {
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

/// Parse a `Range: bytes=START-END` header value.
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

/// Guess the MIME type from file extension / container.
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
        "wmv" => "video/x-ms-wmv",
        "flv" => "video/x-flv",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_range_full() {
        let (start, end) = parse_range_header("bytes=0-999").unwrap();
        assert_eq!(start, 0);
        assert_eq!(end, Some(999));
    }

    #[test]
    fn parse_range_open_end() {
        let (start, end) = parse_range_header("bytes=500-").unwrap();
        assert_eq!(start, 500);
        assert_eq!(end, None);
    }

    #[test]
    fn parse_range_invalid() {
        assert!(parse_range_header("invalid").is_none());
        assert!(parse_range_header("bytes=abc-def").is_none());
    }

    #[test]
    fn content_type_guessing() {
        assert_eq!(guess_content_type("movie.mkv", Some("mkv")), "video/x-matroska");
        assert_eq!(guess_content_type("movie.mp4", None), "video/mp4");
        assert_eq!(guess_content_type("movie.webm", None), "video/webm");
        assert_eq!(guess_content_type("file.xyz", None), "application/octet-stream");
    }
}
