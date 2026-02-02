//! Direct streaming with HTTP range requests.
//!
//! Serves media files directly with support for HTTP range requests.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
};
use sceneforged_common::ids::{ItemId, MediaFileId};
use sceneforged_db::queries::media_files;
use std::io::SeekFrom;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use crate::server::AppContext;

/// Serve media file directly with range request support.
pub async fn stream_file(
    State(ctx): State<AppContext>,
    Path(media_file_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let pool = ctx.db_pool.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let conn = pool.get().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    // Parse ID
    let uuid = media_file_id
        .parse::<uuid::Uuid>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let id = MediaFileId::from(uuid);

    // Get media file info
    let media_file = media_files::get_media_file(&conn, id).map_err(|_| StatusCode::NOT_FOUND)?;

    let file_path = std::path::Path::new(&media_file.file_path);

    // Get file metadata
    let metadata = tokio::fs::metadata(file_path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let file_size = metadata.len();

    // Parse range header if present
    let range = headers
        .get(header::RANGE)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| parse_range_header(s, file_size));

    let content_type = determine_content_type(&media_file.container);

    match range {
        Some((start, end)) => {
            // Partial content response
            let length = end - start + 1;

            let mut file = File::open(file_path)
                .await
                .map_err(|_| StatusCode::NOT_FOUND)?;

            file.seek(SeekFrom::Start(start))
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            let stream = ReaderStream::new(file.take(length));
            let body = Body::from_stream(stream);

            Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_TYPE, content_type)
                .header(header::CONTENT_LENGTH, length.to_string())
                .header(
                    header::CONTENT_RANGE,
                    format!("bytes {}-{}/{}", start, end, file_size),
                )
                .header(header::ACCEPT_RANGES, "bytes")
                .header(header::CACHE_CONTROL, "max-age=31536000")
                .body(body)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        }
        None => {
            // Full file response
            let file = File::open(file_path)
                .await
                .map_err(|_| StatusCode::NOT_FOUND)?;

            let stream = ReaderStream::new(file);
            let body = Body::from_stream(stream);

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
                .header(header::CONTENT_LENGTH, file_size.to_string())
                .header(header::ACCEPT_RANGES, "bytes")
                .header(header::CACHE_CONTROL, "max-age=31536000")
                .body(body)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Parse HTTP Range header.
///
/// Supports formats:
/// - bytes=0-499
/// - bytes=500-999
/// - bytes=500-
/// - bytes=-500 (last 500 bytes)
fn parse_range_header(header: &str, file_size: u64) -> Option<(u64, u64)> {
    let header = header.strip_prefix("bytes=")?;

    let parts: Vec<&str> = header.split('-').collect();
    if parts.len() != 2 {
        return None;
    }

    let start = parts[0].trim();
    let end = parts[1].trim();

    match (start.is_empty(), end.is_empty()) {
        // bytes=-500 (last 500 bytes)
        (true, false) => {
            let suffix_len: u64 = end.parse().ok()?;
            let start = file_size.saturating_sub(suffix_len);
            Some((start, file_size - 1))
        }
        // bytes=500- (from 500 to end)
        (false, true) => {
            let start: u64 = start.parse().ok()?;
            if start >= file_size {
                return None;
            }
            Some((start, file_size - 1))
        }
        // bytes=0-499
        (false, false) => {
            let start: u64 = start.parse().ok()?;
            let end: u64 = end.parse().ok()?;
            if start >= file_size {
                return None;
            }
            let end = end.min(file_size - 1);
            if start > end {
                return None;
            }
            Some((start, end))
        }
        // bytes=- (invalid)
        (true, true) => None,
    }
}

/// Determine content type from container format.
fn determine_content_type(container: &str) -> &'static str {
    match container.to_lowercase().as_str() {
        "mp4" | "m4v" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "avi" => "video/x-msvideo",
        "mov" => "video/quicktime",
        "ts" | "m2ts" => "video/mp2t",
        "m4a" => "audio/mp4",
        "mp3" => "audio/mpeg",
        "flac" => "audio/flac",
        "wav" => "audio/wav",
        _ => "application/octet-stream",
    }
}

/// Stream file by item ID, auto-selecting the best file.
pub async fn stream_item(
    State(ctx): State<AppContext>,
    Path(item_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let pool = ctx.db_pool.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let conn = pool.get().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    // Parse ID
    let uuid = item_id
        .parse::<uuid::Uuid>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let id = ItemId::from(uuid);

    // Try to get universal (Profile B) file first, then fall back to source
    let media_file = media_files::resolve_hls_file(&conn, id)
        .map_err(|_| StatusCode::NOT_FOUND)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Redirect to stream_file with the resolved media_file_id
    let file_path = std::path::Path::new(&media_file.file_path);

    // Get file metadata
    let metadata = tokio::fs::metadata(file_path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let file_size = metadata.len();

    // Parse range header if present
    let range = headers
        .get(header::RANGE)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| parse_range_header(s, file_size));

    let content_type = determine_content_type(&media_file.container);

    match range {
        Some((start, end)) => {
            let length = end - start + 1;

            let mut file = File::open(file_path)
                .await
                .map_err(|_| StatusCode::NOT_FOUND)?;

            file.seek(SeekFrom::Start(start))
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            let stream = ReaderStream::new(file.take(length));
            let body = Body::from_stream(stream);

            Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_TYPE, content_type)
                .header(header::CONTENT_LENGTH, length.to_string())
                .header(
                    header::CONTENT_RANGE,
                    format!("bytes {}-{}/{}", start, end, file_size),
                )
                .header(header::ACCEPT_RANGES, "bytes")
                .header(header::CACHE_CONTROL, "max-age=31536000")
                .body(body)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        }
        None => {
            let file = File::open(file_path)
                .await
                .map_err(|_| StatusCode::NOT_FOUND)?;

            let stream = ReaderStream::new(file);
            let body = Body::from_stream(stream);

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
                .header(header::CONTENT_LENGTH, file_size.to_string())
                .header(header::ACCEPT_RANGES, "bytes")
                .header(header::CACHE_CONTROL, "max-age=31536000")
                .body(body)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_range_header_full_range() {
        assert_eq!(parse_range_header("bytes=0-499", 1000), Some((0, 499)));
    }

    #[test]
    fn test_parse_range_header_open_end() {
        assert_eq!(parse_range_header("bytes=500-", 1000), Some((500, 999)));
    }

    #[test]
    fn test_parse_range_header_suffix() {
        assert_eq!(parse_range_header("bytes=-200", 1000), Some((800, 999)));
    }

    #[test]
    fn test_parse_range_header_clamped() {
        assert_eq!(parse_range_header("bytes=0-2000", 1000), Some((0, 999)));
    }

    #[test]
    fn test_parse_range_header_invalid_start() {
        assert_eq!(parse_range_header("bytes=1500-", 1000), None);
    }

    #[test]
    fn test_parse_range_header_invalid_format() {
        assert_eq!(parse_range_header("bytes=-", 1000), None);
        assert_eq!(parse_range_header("bytes=abc-def", 1000), None);
    }

    #[test]
    fn test_determine_content_type() {
        assert_eq!(determine_content_type("mp4"), "video/mp4");
        assert_eq!(determine_content_type("mkv"), "video/x-matroska");
        assert_eq!(determine_content_type("webm"), "video/webm");
        assert_eq!(determine_content_type("unknown"), "application/octet-stream");
    }
}
