//! Shared streaming helpers: range parsing, content-type guessing, and
//! chunked file serving via `ReaderStream` (Axum safety-net fallback).

use axum::body::Body;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

/// Parse a `Range: bytes=START-END` header value.
///
/// Returns `(start, Option<end>)` where `end` is `None` for open-ended ranges
/// like `bytes=500-`.
pub fn parse_range_header(value: &str) -> Option<(u64, Option<u64>)> {
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
pub fn guess_content_type(file_name: &str, container: Option<&str>) -> &'static str {
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

/// Serve a file using chunked streaming via `ReaderStream`.
///
/// This is the Axum safety-net fallback. Reads are done in 64KB chunks so
/// memory stays bounded regardless of file size. Supports Range requests.
pub async fn serve_file_streaming(
    file_path: &std::path::Path,
    file_name: &str,
    container: Option<&str>,
    range_header: Option<&str>,
) -> Result<Response, sf_core::Error> {
    let metadata = tokio::fs::metadata(file_path)
        .await
        .map_err(|_| sf_core::Error::not_found("file", file_path.display()))?;

    let file_size = metadata.len();
    let content_type = guess_content_type(file_name, container);

    let range = range_header.and_then(parse_range_header);

    match range {
        Some((start, end_opt)) => {
            let end = end_opt.unwrap_or(file_size - 1).min(file_size - 1);
            if start > end || start >= file_size {
                return Ok((
                    StatusCode::RANGE_NOT_SATISFIABLE,
                    [(
                        header::CONTENT_RANGE.as_str(),
                        format!("bytes */{file_size}"),
                    )],
                    Body::empty(),
                )
                    .into_response());
            }

            let length = end - start + 1;

            let mut file = tokio::fs::File::open(file_path).await.map_err(|_| {
                sf_core::Error::not_found("file", file_path.display())
            })?;
            file.seek(std::io::SeekFrom::Start(start))
                .await
                .map_err(|e| sf_core::Error::Internal(format!("Seek failed: {e}")))?;

            // Wrap in a Take to limit reads to exactly `length` bytes.
            let limited = file.take(length);
            let stream = ReaderStream::with_capacity(limited, 64 * 1024);
            let body = Body::from_stream(stream);

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
                body,
            )
                .into_response())
        }
        None => {
            let file = tokio::fs::File::open(file_path).await.map_err(|_| {
                sf_core::Error::not_found("file", file_path.display())
            })?;

            let stream = ReaderStream::with_capacity(file, 64 * 1024);
            let body = Body::from_stream(stream);

            Ok((
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE.as_str(), content_type.to_string()),
                    (header::CONTENT_LENGTH.as_str(), file_size.to_string()),
                    (header::ACCEPT_RANGES.as_str(), "bytes".to_string()),
                ],
                body,
            )
                .into_response())
        }
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
    fn parse_range_with_end() {
        let (start, end) = parse_range_header("bytes=10-20").unwrap();
        assert_eq!(start, 10);
        assert_eq!(end, Some(20));
    }

    #[test]
    fn content_type_guessing() {
        assert_eq!(guess_content_type("movie.mkv", Some("mkv")), "video/x-matroska");
        assert_eq!(guess_content_type("movie.mp4", None), "video/mp4");
        assert_eq!(guess_content_type("movie.webm", None), "video/webm");
        assert_eq!(guess_content_type("file.xyz", None), "application/octet-stream");
    }

    #[test]
    fn content_type_all_variants() {
        assert_eq!(guess_content_type("x.m4v", Some("m4v")), "video/mp4");
        assert_eq!(guess_content_type("x.avi", Some("avi")), "video/x-msvideo");
        assert_eq!(guess_content_type("x.ts", Some("ts")), "video/mp2t");
        assert_eq!(guess_content_type("x.mov", Some("mov")), "video/quicktime");
        assert_eq!(guess_content_type("x.wmv", Some("wmv")), "video/x-ms-wmv");
        assert_eq!(guess_content_type("x.flv", Some("flv")), "video/x-flv");
    }

    #[test]
    fn content_type_container_overrides_extension() {
        assert_eq!(guess_content_type("movie.mkv", Some("mp4")), "video/mp4");
    }
}
