//! Integration tests for the sendfile-based HLS segment serving.
//!
//! Verifies that the custom TCP accept loop correctly routes segment requests
//! to the sendfile handler and that non-segment requests still go through
//! Axum. Uses a synthetic `PreparedMedia` with a temp file to avoid needing
//! a real MP4.

mod common;

use std::io::Write;
use std::sync::Arc;

use common::TestHarness;
use sf_media::{DataRange, PrecomputedSegment, PreparedMedia};

/// Create a synthetic PreparedMedia backed by a temp file containing known data.
/// Returns the PreparedMedia and the temp file handle (to keep it alive).
fn synthetic_prepared_media() -> (PreparedMedia, tempfile::NamedTempFile) {
    // Create a temp file with known content: 256 bytes of video data + 128 bytes of audio data.
    let mut tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");

    // Write video data: bytes 0..256 = repeating pattern 0xAA.
    let video_data = vec![0xAAu8; 256];
    tmp.write_all(&video_data).unwrap();

    // Write audio data: bytes 256..384 = repeating pattern 0xBB.
    let audio_data = vec![0xBBu8; 128];
    tmp.write_all(&audio_data).unwrap();

    tmp.flush().unwrap();

    // Build a fake moof + mdat header.
    let moof_bytes = vec![0x01, 0x02, 0x03, 0x04]; // 4 bytes of fake moof
    let mdat_header = vec![0x05, 0x06, 0x07, 0x08]; // 4 bytes of fake mdat header

    let segment = PrecomputedSegment {
        index: 0,
        start_time_secs: 0.0,
        duration_secs: 2.0,
        moof_bytes: moof_bytes.clone(),
        mdat_header: mdat_header.clone(),
        video_data_ranges: vec![DataRange {
            file_offset: 0,
            length: 256,
        }],
        audio_data_ranges: vec![DataRange {
            file_offset: 256,
            length: 128,
        }],
        data_length: 384, // 256 + 128
    };

    let prepared = PreparedMedia {
        file_path: tmp.path().to_path_buf(),
        width: 1920,
        height: 1080,
        duration_secs: 2.0,
        init_segment: vec![0xFF; 32], // Fake init segment.
        variant_playlist: "#EXTM3U\n#EXT-X-TARGETDURATION:2\n".to_string(),
        segments: vec![segment],
        target_duration: 2,
    };

    (prepared, tmp)
}

// ---------------------------------------------------------------------------
// Segment request served via sendfile path
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sendfile_serves_segment_correctly() {
    let (harness, addr) = TestHarness::with_sendfile_server().await;

    let mf_id = sf_core::MediaFileId::new();
    let (prepared, _tmp) = synthetic_prepared_media();

    // Expected body: moof_bytes + mdat_header + video_data + audio_data.
    let mut expected_body = Vec::new();
    expected_body.extend_from_slice(&prepared.segments[0].moof_bytes);
    expected_body.extend_from_slice(&prepared.segments[0].mdat_header);
    expected_body.extend_from_slice(&vec![0xAAu8; 256]);
    expected_body.extend_from_slice(&vec![0xBBu8; 128]);

    // Insert into HLS cache.
    harness.ctx.hls_cache.insert(mf_id, (Arc::new(prepared), std::time::Instant::now()));

    // Request the segment.
    let client = reqwest::Client::new();
    let url = format!("http://{addr}/api/stream/{mf_id}/segment_0.m4s");
    let resp = client.get(&url).send().await.expect("request failed");

    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap().to_str().unwrap(),
        "video/iso.segment"
    );
    assert_eq!(
        resp.headers()
            .get("connection")
            .unwrap()
            .to_str()
            .unwrap(),
        "close"
    );

    let body = resp.bytes().await.unwrap();
    assert_eq!(body.len(), expected_body.len());
    assert_eq!(body.as_ref(), expected_body.as_slice());
}

// ---------------------------------------------------------------------------
// Non-segment requests still go through Axum
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sendfile_server_passes_non_segment_to_axum() {
    let (_harness, addr) = TestHarness::with_sendfile_server().await;

    // Health check should still work through the Axum path.
    let resp = reqwest::get(format!("http://{addr}/health"))
        .await
        .expect("request failed");
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "ok");
}

#[tokio::test]
async fn sendfile_server_serves_init_mp4_via_axum() {
    let (harness, addr) = TestHarness::with_sendfile_server().await;

    let mf_id = sf_core::MediaFileId::new();
    let (prepared, _tmp) = synthetic_prepared_media();
    let expected_init = prepared.init_segment.clone();

    harness.ctx.hls_cache.insert(mf_id, (Arc::new(prepared), std::time::Instant::now()));

    // init.mp4 should go through Axum, not sendfile.
    let resp = reqwest::get(format!("http://{addr}/api/stream/{mf_id}/init.mp4"))
        .await
        .expect("request failed");
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.bytes().await.unwrap().as_ref(), expected_init.as_slice());
}

#[tokio::test]
async fn sendfile_server_serves_playlist_via_axum() {
    let (harness, addr) = TestHarness::with_sendfile_server().await;

    let mf_id = sf_core::MediaFileId::new();
    let (prepared, _tmp) = synthetic_prepared_media();
    let expected_playlist = prepared.variant_playlist.clone();

    harness.ctx.hls_cache.insert(mf_id, (Arc::new(prepared), std::time::Instant::now()));

    // Playlist should go through Axum, not sendfile.
    let resp = reqwest::get(format!("http://{addr}/api/stream/{mf_id}/index.m3u8"))
        .await
        .expect("request failed");
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), expected_playlist);
}

// ---------------------------------------------------------------------------
// 404 for missing segment / media file
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sendfile_returns_404_for_missing_media_file() {
    let (_harness, addr) = TestHarness::with_sendfile_server().await;

    let fake_id = sf_core::MediaFileId::new();
    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "http://{addr}/api/stream/{fake_id}/segment_0.m4s"
        ))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn sendfile_returns_404_for_missing_segment_index() {
    let (harness, addr) = TestHarness::with_sendfile_server().await;

    let mf_id = sf_core::MediaFileId::new();
    let (prepared, _tmp) = synthetic_prepared_media();
    harness.ctx.hls_cache.insert(mf_id, (Arc::new(prepared), std::time::Instant::now()));

    // Segment index 99 does not exist (only index 0).
    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "http://{addr}/api/stream/{mf_id}/segment_99.m4s"
        ))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), 404);
}

// ---------------------------------------------------------------------------
// Auth rejection on sendfile path
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sendfile_rejects_unauthenticated_when_auth_enabled() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    config.auth.api_key = Some("test-key".into());

    let (harness, addr) = TestHarness::with_sendfile_server_config(config).await;

    let mf_id = sf_core::MediaFileId::new();
    let (prepared, _tmp) = synthetic_prepared_media();
    harness.ctx.hls_cache.insert(mf_id, (Arc::new(prepared), std::time::Instant::now()));

    // Request without credentials should get 401.
    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "http://{addr}/api/stream/{mf_id}/segment_0.m4s"
        ))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn sendfile_accepts_authenticated_when_auth_enabled() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    config.auth.api_key = Some("test-key".into());

    let (harness, addr) = TestHarness::with_sendfile_server_config(config).await;

    let mf_id = sf_core::MediaFileId::new();
    let (prepared, _tmp) = synthetic_prepared_media();
    harness.ctx.hls_cache.insert(mf_id, (Arc::new(prepared), std::time::Instant::now()));

    // Request with correct API key should succeed.
    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "http://{addr}/api/stream/{mf_id}/segment_0.m4s"
        ))
        .header("Authorization", "Bearer test-key")
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), 200);
    assert!(!resp.bytes().await.unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// Multiple segments in a PreparedMedia
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sendfile_serves_multiple_segments() {
    let (harness, addr) = TestHarness::with_sendfile_server().await;

    let mf_id = sf_core::MediaFileId::new();

    // Create a temp file with two distinct regions.
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    let seg0_video = vec![0x11u8; 100];
    let seg1_video = vec![0x22u8; 200];
    tmp.write_all(&seg0_video).unwrap();
    tmp.write_all(&seg1_video).unwrap();
    tmp.flush().unwrap();

    let moof0 = vec![0xA0, 0xA1];
    let mdat0 = vec![0xB0, 0xB1];
    let moof1 = vec![0xC0, 0xC1, 0xC2];
    let mdat1 = vec![0xD0];

    let seg0 = PrecomputedSegment {
        index: 0,
        start_time_secs: 0.0,
        duration_secs: 2.0,
        moof_bytes: moof0.clone(),
        mdat_header: mdat0.clone(),
        video_data_ranges: vec![DataRange {
            file_offset: 0,
            length: 100,
        }],
        audio_data_ranges: vec![],
        data_length: 100,
    };

    let seg1 = PrecomputedSegment {
        index: 1,
        start_time_secs: 2.0,
        duration_secs: 2.0,
        moof_bytes: moof1.clone(),
        mdat_header: mdat1.clone(),
        video_data_ranges: vec![DataRange {
            file_offset: 100,
            length: 200,
        }],
        audio_data_ranges: vec![],
        data_length: 200,
    };

    let prepared = PreparedMedia {
        file_path: tmp.path().to_path_buf(),
        width: 1920,
        height: 1080,
        duration_secs: 4.0,
        init_segment: vec![],
        variant_playlist: String::new(),
        segments: vec![seg0, seg1],
        target_duration: 2,
    };

    harness.ctx.hls_cache.insert(mf_id, (Arc::new(prepared), std::time::Instant::now()));

    let client = reqwest::Client::new();

    // Segment 0.
    let resp = client
        .get(format!("http://{addr}/api/stream/{mf_id}/segment_0.m4s"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.bytes().await.unwrap();
    let mut expected0 = Vec::new();
    expected0.extend_from_slice(&moof0);
    expected0.extend_from_slice(&mdat0);
    expected0.extend_from_slice(&seg0_video);
    assert_eq!(body.as_ref(), expected0.as_slice());

    // Segment 1.
    let resp = client
        .get(format!("http://{addr}/api/stream/{mf_id}/segment_1.m4s"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.bytes().await.unwrap();
    let mut expected1 = Vec::new();
    expected1.extend_from_slice(&moof1);
    expected1.extend_from_slice(&mdat1);
    expected1.extend_from_slice(&seg1_video);
    assert_eq!(body.as_ref(), expected1.as_slice());
}
