//! HLS streaming handlers.
//!
//! Serves HLS playlists and segments for media files.
//!
//! # Zero-Copy Optimization
//!
//! Media segments use a streaming approach to minimize memory copies:
//! - moof header is generated once and converted to Bytes
//! - File data is streamed directly via ReaderStream
//! - The two streams are chained, avoiding buffer concatenation

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::Response,
};
use bytes::Bytes;
use futures::stream::{self, StreamExt};
use sceneforged_common::ids::MediaFileId;
use sceneforged_db::queries::media_files;
use sceneforged_media::{
    hls::{MediaPlaylist, StreamInfo},
    mp4::Mp4File,
    segment_map::SegmentMapBuilder,
};
use std::io::SeekFrom;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use super::segment_cache::SegmentCache;
use crate::server::AppContext;

/// Get a shared segment cache from AppContext (uses global static for now).
fn get_segment_cache() -> Arc<SegmentCache> {
    use std::sync::OnceLock;
    static CACHE: OnceLock<Arc<SegmentCache>> = OnceLock::new();
    CACHE
        .get_or_init(|| Arc::new(SegmentCache::default()))
        .clone()
}

/// Serve HLS master playlist (for adaptive bitrate).
pub async fn master_playlist(
    State(ctx): State<AppContext>,
    Path(media_file_id): Path<String>,
) -> Result<Response, StatusCode> {
    let pool = ctx
        .db_pool
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let conn = pool.get().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    // Parse ID
    let uuid = media_file_id
        .parse::<uuid::Uuid>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let id = MediaFileId::from(uuid);

    // Get media file info
    let media_file = media_files::get_media_file(&conn, id).map_err(|_| StatusCode::NOT_FOUND)?;

    // For now, generate a simple single-stream playlist
    // Use relative URLs to avoid issues with 0.0.0.0 bind address
    let stream = StreamInfo {
        id: media_file_id.clone(),
        uri: format!("/api/stream/{}/playlist.m3u8", media_file_id),
        bandwidth: media_file.bit_rate.unwrap_or(5_000_000) as u32,
        width: media_file.width.unwrap_or(1920) as u32,
        height: media_file.height.unwrap_or(1080) as u32,
        codecs: format_codec_string(&media_file.video_codec, &media_file.audio_codec),
        frame_rate: None,
        audio_group: None,
    };

    let master = sceneforged_media::hls::MasterPlaylist::new().add_stream(stream);
    let playlist = master.render();

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")
        .header(header::CACHE_CONTROL, "max-age=60")
        .body(Body::from(playlist))
        .unwrap())
}

/// Serve HLS media playlist (segment list).
pub async fn media_playlist(
    State(ctx): State<AppContext>,
    Path(media_file_id): Path<String>,
) -> Result<Response, StatusCode> {
    let pool = ctx
        .db_pool
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let conn = pool.get().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    // Parse ID
    let uuid = media_file_id
        .parse::<uuid::Uuid>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let id = MediaFileId::from(uuid);

    // Get media file info
    let media_file = media_files::get_media_file(&conn, id).map_err(|_| StatusCode::NOT_FOUND)?;

    let file_path = std::path::Path::new(&media_file.file_path);

    let segment_cache = get_segment_cache();

    // Get or compute segment map
    let segment_map = segment_cache
        .get_or_insert(&media_file_id, file_path, |path| {
            let mp4 = Mp4File::open(path).ok()?;
            let video_track = mp4.video_track.as_ref()?;
            Some(
                SegmentMapBuilder::new()
                    .timescale(video_track.timescale)
                    .target_duration(6.0)
                    .build(&video_track.sample_table),
            )
        })
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Generate media playlist with relative URLs to avoid 0.0.0.0 bind address issues
    let playlist = MediaPlaylist::from_segment_map(&segment_map, "/api", &media_file_id);
    let m3u8 = playlist.render();

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")
        .header(header::CACHE_CONTROL, "max-age=60")
        .body(Body::from(m3u8))
        .unwrap())
}

/// Serve HLS init segment (ftyp + moov).
pub async fn init_segment(
    State(ctx): State<AppContext>,
    Path(media_file_id): Path<String>,
) -> Result<Response, StatusCode> {
    let pool = ctx
        .db_pool
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let conn = pool.get().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    // Parse ID
    let uuid = media_file_id
        .parse::<uuid::Uuid>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let id = MediaFileId::from(uuid);

    // Get media file
    let media_file = media_files::get_media_file(&conn, id).map_err(|_| StatusCode::NOT_FOUND)?;

    let file_path = std::path::Path::new(&media_file.file_path);

    // Parse MP4 to get init segment data
    let mp4 = Mp4File::open(file_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Build init segment
    let video_track = mp4.video_track.as_ref().ok_or(StatusCode::NOT_FOUND)?;

    let mut init_builder = sceneforged_media::fmp4::InitSegmentBuilder::new()
        .timescale(video_track.timescale)
        .duration(video_track.duration);

    if let (Some(width), Some(height)) = (video_track.width, video_track.height) {
        init_builder = init_builder.dimensions(width, height);
    }

    let init_segment = init_builder.build();
    let init_data = init_segment.data;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "video/mp4")
        .header(header::CACHE_CONTROL, "max-age=31536000, immutable")
        .body(Body::from(init_data))
        .unwrap())
}

/// Serve HLS media segment.
///
/// Uses zero-copy streaming: moof header is sent first, then file data is streamed
/// directly without buffering the entire segment in memory.
pub async fn media_segment(
    State(ctx): State<AppContext>,
    Path((media_file_id, segment_index_str)): Path<(String, String)>,
) -> Result<Response, StatusCode> {
    let pool = ctx
        .db_pool
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let conn = pool.get().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    // Parse segment index, stripping .m4s extension if present
    let segment_index: u32 = segment_index_str
        .trim_end_matches(".m4s")
        .parse()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    // Parse ID
    let uuid = media_file_id
        .parse::<uuid::Uuid>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let id = MediaFileId::from(uuid);

    // Get media file
    let media_file = media_files::get_media_file(&conn, id).map_err(|_| StatusCode::NOT_FOUND)?;

    let file_path = std::path::Path::new(&media_file.file_path);
    let segment_cache = get_segment_cache();

    // Get segment map
    let segment_map = segment_cache
        .get_or_insert(&media_file_id, file_path, |path| {
            let mp4 = Mp4File::open(path).ok()?;
            let video_track = mp4.video_track.as_ref()?;
            Some(
                SegmentMapBuilder::new()
                    .timescale(video_track.timescale)
                    .target_duration(6.0)
                    .build(&video_track.sample_table),
            )
        })
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get segment info
    let segment = segment_map
        .get_segment(segment_index)
        .ok_or(StatusCode::NOT_FOUND)?;

    // Build moof + mdat header (this is the only part we need to generate)
    let mp4 = Mp4File::open(file_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let video_track = mp4.video_track.as_ref().ok_or(StatusCode::NOT_FOUND)?;

    // Get samples for this segment
    let samples: Vec<_> = (segment.start_sample..segment.end_sample)
        .filter_map(|i| video_track.sample_table.get(i))
        .cloned()
        .collect();

    // Calculate base media decode time
    let base_decode_time = samples.first().map(|s| s.dts).unwrap_or(0);

    let moof_builder = sceneforged_media::fmp4::MoofBuilder::new(segment_index + 1, 1)
        .base_media_decode_time(base_decode_time);

    // Build moof header (small, ~100-500 bytes typically)
    let moof_mdat_header = moof_builder.build(&samples);
    let header_len = moof_mdat_header.len();
    let header_bytes = Bytes::from(moof_mdat_header);

    // Calculate total content length for Content-Length header
    let total_size = header_len as u64 + segment.data_size;

    // Open file and seek to segment data
    let mut file = tokio::fs::File::open(file_path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    file.seek(SeekFrom::Start(segment.data_offset))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create bounded reader for just this segment's data
    let segment_reader = file.take(segment.data_size);

    // Stream: first the moof header, then the file data
    // This avoids the extend_from_slice copy!
    let header_stream = stream::once(async move { Ok::<_, std::io::Error>(header_bytes) });
    let file_stream = ReaderStream::new(segment_reader);

    // Chain the streams together
    let combined_stream = header_stream.chain(file_stream);
    let body = Body::from_stream(combined_stream);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "video/mp4")
        .header(header::CONTENT_LENGTH, total_size.to_string())
        .header(header::CACHE_CONTROL, "max-age=31536000, immutable")
        .body(body)
        .unwrap())
}

fn format_codec_string(video: &Option<String>, audio: &Option<String>) -> String {
    match (video.as_deref(), audio.as_deref()) {
        (Some(v), Some(a)) => format!("{},{}", map_video_codec(v), map_audio_codec(a)),
        (Some(v), None) => map_video_codec(v).to_string(),
        (None, Some(a)) => map_audio_codec(a).to_string(),
        (None, None) => "avc1.64001f,mp4a.40.2".to_string(),
    }
}

fn map_video_codec(codec: &str) -> &str {
    match codec.to_lowercase().as_str() {
        "h264" | "avc" | "avc1" => "avc1.64001f",
        "h265" | "hevc" | "hvc1" => "hvc1.1.6.L93.B0",
        "av1" => "av01.0.08M.08",
        "vp9" => "vp09.00.10.08",
        _ => "avc1.64001f",
    }
}

fn map_audio_codec(codec: &str) -> &str {
    match codec.to_lowercase().as_str() {
        "aac" | "aac-lc" => "mp4a.40.2",
        "he-aac" | "aac-he" => "mp4a.40.5",
        "ac3" => "ac-3",
        "eac3" | "e-ac3" => "ec-3",
        "opus" => "Opus",
        "flac" => "fLaC",
        _ => "mp4a.40.2",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_codec_string() {
        let result = format_codec_string(&Some("h264".to_string()), &Some("aac".to_string()));
        assert!(result.contains("avc1"));
        assert!(result.contains("mp4a"));
    }

    #[test]
    fn test_map_video_codec() {
        assert_eq!(map_video_codec("h264"), "avc1.64001f");
        assert_eq!(map_video_codec("hevc"), "hvc1.1.6.L93.B0");
        assert_eq!(map_video_codec("av1"), "av01.0.08M.08");
    }

    #[test]
    fn test_map_audio_codec() {
        assert_eq!(map_audio_codec("aac"), "mp4a.40.2");
        assert_eq!(map_audio_codec("ac3"), "ac-3");
        assert_eq!(map_audio_codec("eac3"), "ec-3");
    }
}
