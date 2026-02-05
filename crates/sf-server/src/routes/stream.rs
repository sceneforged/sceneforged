//! Streaming route handlers: HLS and direct file streaming.
//!
//! HLS serves pre-generated fMP4 segments from disk.
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
            // No range — return full file for small files, or signal accept-ranges.
            // For large files (>10MB), we still read the whole thing but set Accept-Ranges
            // so clients know they can do range requests next time.
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
