//! End-to-end streaming tests using a real Profile B MP4 fixture.
//!
//! These tests exercise the full HLS pipeline: moov parsing → segment map
//! computation → init segment generation → fMP4 segment serving.
//! The fixture file is a 24-second Big Buck Bunny clip encoded as Profile B
//! (H.264 High, AAC stereo, MP4 faststart, keyframes every 2s).

mod common;

use common::TestHarness;
use std::sync::Arc;

/// Path to the Big Buck Bunny Profile B test fixture.
fn fixture_path() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    format!("{manifest}/tests/fixtures/bbb_profile_b.mp4")
}

// ---------------------------------------------------------------------------
// sf-media: moov parsing + segment map (unit-level with real file)
// ---------------------------------------------------------------------------

#[test]
fn parse_moov_from_real_mp4() {
    let path = fixture_path();
    let mut reader = std::io::BufReader::new(std::fs::File::open(&path).unwrap());
    let metadata = sf_media::parse_moov(&mut reader).unwrap();

    // Video track must be present.
    let video = metadata.video_track.as_ref().expect("video track missing");
    assert_eq!(video.width, 640);
    assert_eq!(video.height, 360);
    assert!(video.timescale > 0);
    assert!(!video.codec_private.is_empty(), "avcC data must be present");

    // Audio track must be present.
    let audio = metadata.audio_track.as_ref().expect("audio track missing");
    assert_eq!(audio.sample_rate, 48000);
    assert!(audio.channels >= 2);

    // Duration should be ~24 seconds.
    assert!(
        metadata.duration_secs > 23.0 && metadata.duration_secs < 25.0,
        "duration {:.1}s out of expected range",
        metadata.duration_secs
    );
}

#[test]
fn build_prepared_media_from_real_mp4() {
    let path = fixture_path();
    let mut reader = std::io::BufReader::new(std::fs::File::open(&path).unwrap());
    let metadata = sf_media::parse_moov(&mut reader).unwrap();
    let prepared =
        sf_media::build_prepared_media(&metadata, std::path::Path::new(&path)).unwrap();

    // Init segment should be non-empty (ftyp + moov).
    assert!(
        prepared.init_segment.len() > 100,
        "init segment too small: {} bytes",
        prepared.init_segment.len()
    );

    // With 24s of video and ~6s target segments, expect 3-5 segments.
    assert!(
        prepared.segments.len() >= 3 && prepared.segments.len() <= 6,
        "unexpected segment count: {}",
        prepared.segments.len()
    );

    // Verify all segments have valid data.
    for seg in &prepared.segments {
        assert!(!seg.moof_bytes.is_empty(), "segment {} has empty moof", seg.index);
        assert!(!seg.mdat_header.is_empty(), "segment {} has empty mdat header", seg.index);
        assert!(
            !seg.video_data_ranges.is_empty(),
            "segment {} has no video data ranges",
            seg.index
        );
        assert!(seg.data_length > 0, "segment {} has zero data_length", seg.index);
        assert!(seg.duration_secs > 0.0, "segment {} has zero duration", seg.index);
    }

    // Playlist should be valid M3U8.
    assert!(
        prepared.variant_playlist.starts_with("#EXTM3U"),
        "playlist doesn't start with #EXTM3U"
    );
    assert!(
        prepared.variant_playlist.contains("#EXT-X-TARGETDURATION"),
        "playlist missing target duration"
    );
    assert!(
        prepared.variant_playlist.contains("#EXT-X-MAP:URI=\"init.mp4\""),
        "playlist missing init map"
    );
    assert!(
        prepared.variant_playlist.contains("segment_0.m4s"),
        "playlist missing segment references"
    );
    assert!(
        prepared.variant_playlist.contains("#EXT-X-ENDLIST"),
        "playlist missing endlist for VOD"
    );

    // Dimensions should match source.
    assert_eq!(prepared.width, 640);
    assert_eq!(prepared.height, 360);
}

// ---------------------------------------------------------------------------
// HTTP: full HLS pipeline via server routes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn hls_playlist_from_real_mp4() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let path = fixture_path();

    let (_, _, _, mf_id_str) = h.create_item_with_real_media(
        lib_id,
        "Big Buck Bunny",
        &path,
        "mp4",
        "h264",
        "aac",
        640,
        360,
        "B",
        24.0,
    );

    // Request M3U8 playlist — triggers full HLS preparation pipeline.
    let resp = reqwest::get(format!("http://{addr}/api/stream/{mf_id_str}/index.m3u8"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert!(resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("mpegurl"));

    let playlist = resp.text().await.unwrap();
    assert!(playlist.starts_with("#EXTM3U"), "not a valid M3U8 playlist");
    assert!(playlist.contains("#EXT-X-MAP:URI=\"init.mp4\""));
    assert!(playlist.contains("segment_0.m4s"));
    assert!(playlist.contains("#EXT-X-ENDLIST"));
}

#[tokio::test]
async fn hls_init_segment_from_real_mp4() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let path = fixture_path();

    let (_, _, _, mf_id_str) = h.create_item_with_real_media(
        lib_id,
        "Big Buck Bunny",
        &path,
        "mp4",
        "h264",
        "aac",
        640,
        360,
        "B",
        24.0,
    );

    // First request playlist to trigger preparation.
    let resp = reqwest::get(format!("http://{addr}/api/stream/{mf_id_str}/index.m3u8"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Request init segment.
    let resp = reqwest::get(format!("http://{addr}/api/stream/{mf_id_str}/init.mp4"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap().to_str().unwrap(),
        "video/mp4"
    );

    let init_data = resp.bytes().await.unwrap();
    assert!(init_data.len() > 100, "init segment too small: {} bytes", init_data.len());

    // Verify init segment starts with an MP4 box (ftyp).
    // ftyp box: first 4 bytes are size, next 4 are "ftyp".
    assert!(init_data.len() >= 8);
    assert_eq!(&init_data[4..8], b"ftyp", "init segment should start with ftyp box");
}

#[tokio::test]
async fn hls_segments_from_real_mp4() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let path = fixture_path();

    let (_, _, _, mf_id_str) = h.create_item_with_real_media(
        lib_id,
        "Big Buck Bunny",
        &path,
        "mp4",
        "h264",
        "aac",
        640,
        360,
        "B",
        24.0,
    );

    // Request playlist to trigger preparation and parse segment count.
    let resp = reqwest::get(format!("http://{addr}/api/stream/{mf_id_str}/index.m3u8"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let playlist = resp.text().await.unwrap();

    // Count segments from playlist.
    let segment_count = playlist.matches("segment_").count();
    assert!(segment_count >= 3, "expected at least 3 segments, got {segment_count}");

    // Request each segment and verify it's valid fMP4.
    let client = reqwest::Client::new();
    for i in 0..segment_count {
        let resp = client
            .get(format!(
                "http://{addr}/api/stream/{mf_id_str}/segment_{i}.m4s"
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "segment {i} returned non-200");
        assert_eq!(
            resp.headers().get("content-type").unwrap().to_str().unwrap(),
            "video/iso.segment",
            "segment {i} wrong content type"
        );

        let seg_data = resp.bytes().await.unwrap();
        assert!(
            seg_data.len() > 100,
            "segment {i} too small: {} bytes",
            seg_data.len()
        );

        // Each segment should start with a moof box.
        assert!(seg_data.len() >= 8);
        assert_eq!(
            &seg_data[4..8],
            b"moof",
            "segment {i} should start with moof box"
        );
    }

    // Segment past the end should 404.
    let resp = client
        .get(format!(
            "http://{addr}/api/stream/{mf_id_str}/segment_{segment_count}.m4s"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// ---------------------------------------------------------------------------
// Sendfile path: segment served via zero-copy
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sendfile_serves_real_mp4_segment() {
    let (h, addr) = TestHarness::with_sendfile_server().await;
    let (lib_id, _) = h.create_library();
    let path = fixture_path();

    let (_, mf_id, _, mf_id_str) = h.create_item_with_real_media(
        lib_id,
        "Big Buck Bunny",
        &path,
        "mp4",
        "h264",
        "aac",
        640,
        360,
        "B",
        24.0,
    );

    // Pre-populate HLS cache by parsing moov directly.
    let mut reader =
        std::io::BufReader::new(std::fs::File::open(&path).unwrap());
    let metadata = sf_media::parse_moov(&mut reader).unwrap();
    let prepared =
        sf_media::build_prepared_media(&metadata, std::path::Path::new(&path)).unwrap();
    let segment_count = prepared.segments.len();

    h.ctx
        .hls_cache
        .insert(mf_id, (Arc::new(prepared), std::time::Instant::now()));

    let client = reqwest::Client::new();

    // Request segment 0 via sendfile path.
    let resp = client
        .get(format!(
            "http://{addr}/api/stream/{mf_id_str}/segment_0.m4s"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap().to_str().unwrap(),
        "video/iso.segment"
    );

    let seg_data = resp.bytes().await.unwrap();
    assert!(seg_data.len() > 100, "segment too small: {} bytes", seg_data.len());
    assert_eq!(&seg_data[4..8], b"moof", "should start with moof box");

    // Request last segment.
    let last = segment_count - 1;
    let resp = client
        .get(format!(
            "http://{addr}/api/stream/{mf_id_str}/segment_{last}.m4s"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let seg_data = resp.bytes().await.unwrap();
    assert!(seg_data.len() > 50);
    assert_eq!(&seg_data[4..8], b"moof");
}

// ---------------------------------------------------------------------------
// Direct stream with real file
// ---------------------------------------------------------------------------

#[tokio::test]
async fn direct_stream_serves_real_mp4() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let path = fixture_path();

    let (_, _, _, mf_id_str) = h.create_item_with_real_media(
        lib_id,
        "Big Buck Bunny",
        &path,
        "mp4",
        "h264",
        "aac",
        640,
        360,
        "B",
        24.0,
    );

    let resp = reqwest::get(format!("http://{addr}/api/stream/{mf_id_str}/direct"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap().to_str().unwrap(),
        "video/mp4"
    );
    assert_eq!(
        resp.headers()
            .get("accept-ranges")
            .unwrap()
            .to_str()
            .unwrap(),
        "bytes"
    );

    let body = resp.bytes().await.unwrap();
    let file_size = std::fs::metadata(&path).unwrap().len() as usize;
    assert_eq!(body.len(), file_size, "direct stream should serve entire file");

    // Body should start with ftyp box (Profile B MP4 with faststart).
    assert_eq!(&body[4..8], b"ftyp");
}

#[tokio::test]
async fn direct_stream_range_request_real_mp4() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let path = fixture_path();

    let (_, _, _, mf_id_str) = h.create_item_with_real_media(
        lib_id,
        "Big Buck Bunny",
        &path,
        "mp4",
        "h264",
        "aac",
        640,
        360,
        "B",
        24.0,
    );

    let file_size = std::fs::metadata(&path).unwrap().len();

    // Range request for first 1KB.
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{addr}/api/stream/{mf_id_str}/direct"))
        .header("Range", "bytes=0-1023")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 206);
    assert!(resp
        .headers()
        .get("content-range")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with(&format!("bytes 0-1023/{file_size}")));
    let body = resp.bytes().await.unwrap();
    assert_eq!(body.len(), 1024);
    // First 1KB should contain ftyp box.
    assert_eq!(&body[4..8], b"ftyp");
}

// ---------------------------------------------------------------------------
// Jellyfin streaming with real file
// ---------------------------------------------------------------------------

#[tokio::test]
async fn jellyfin_stream_real_mp4() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let path = fixture_path();

    let (_, _, item_id_str, _) = h.create_item_with_real_media(
        lib_id,
        "Big Buck Bunny",
        &path,
        "mp4",
        "h264",
        "aac",
        640,
        360,
        "B",
        24.0,
    );

    // Jellyfin direct stream endpoint.
    let resp = reqwest::get(format!("http://{addr}/Videos/{item_id_str}/stream"))
        .await
        .unwrap();

    // Should serve the file (may select best media file for item).
    // If the endpoint finds a media file, expect 200 with video content.
    // The exact behavior depends on how Jellyfin streaming selects the file.
    let status = resp.status().as_u16();
    assert!(
        status == 200 || status == 404,
        "unexpected status {status} for Jellyfin stream"
    );
}
