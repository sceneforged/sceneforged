//! Integration tests for streaming routes (direct stream, HLS error paths).

mod common;

use common::TestHarness;

#[tokio::test]
async fn direct_stream_serves_file() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();

    // Create an item with a media file pointing to a real temp file.
    let dir = tempfile::tempdir().unwrap();
    let video_path = dir.path().join("test_video.mp4");
    let video_data = vec![0u8; 1024]; // 1KB fake video
    std::fs::write(&video_path, &video_data).unwrap();

    let conn = h.conn();
    let item = sf_db::queries::items::create_item(
        &conn, lib_id, "movie", "StreamTest", None, Some(2024), None,
        Some(120), None, None, None, None, None,
    )
    .unwrap();
    let mf = sf_db::queries::media_files::create_media_file(
        &conn,
        item.id,
        video_path.to_str().unwrap(),
        "test_video.mp4",
        1024,
        Some("mp4"),
        Some("h264"),
        Some("aac"),
        Some(1920),
        Some(1080),
        None,
        false,
        None,
        "source",
        "C",
        Some(120.0),
    )
    .unwrap();
    let mf_id_str = mf.id.to_string();

    // Full file request (no Range header).
    let resp = reqwest::get(format!(
        "http://{addr}/api/stream/{mf_id_str}/direct"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap().to_str().unwrap(),
        "video/mp4"
    );
    assert_eq!(
        resp.headers().get("accept-ranges").unwrap().to_str().unwrap(),
        "bytes"
    );
    let body = resp.bytes().await.unwrap();
    assert_eq!(body.len(), 1024);
}

#[tokio::test]
async fn direct_stream_range_request() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();

    let dir = tempfile::tempdir().unwrap();
    let video_path = dir.path().join("range_test.mp4");
    let video_data: Vec<u8> = (0..=255u8).cycle().take(2048).collect();
    std::fs::write(&video_path, &video_data).unwrap();

    let conn = h.conn();
    let item = sf_db::queries::items::create_item(
        &conn, lib_id, "movie", "RangeTest", None, Some(2024), None,
        Some(120), None, None, None, None, None,
    )
    .unwrap();
    let mf = sf_db::queries::media_files::create_media_file(
        &conn,
        item.id,
        video_path.to_str().unwrap(),
        "range_test.mp4",
        2048,
        Some("mp4"),
        Some("h264"),
        Some("aac"),
        Some(1920),
        Some(1080),
        None,
        false,
        None,
        "source",
        "C",
        Some(120.0),
    )
    .unwrap();
    let mf_id_str = mf.id.to_string();

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{addr}/api/stream/{mf_id_str}/direct"))
        .header("Range", "bytes=100-199")
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
        .starts_with("bytes 100-199/2048"));
    let body = resp.bytes().await.unwrap();
    assert_eq!(body.len(), 100);
}

#[tokio::test]
async fn direct_stream_open_range() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();

    let dir = tempfile::tempdir().unwrap();
    let video_path = dir.path().join("open_range.mp4");
    std::fs::write(&video_path, vec![42u8; 500]).unwrap();

    let conn = h.conn();
    let item = sf_db::queries::items::create_item(
        &conn, lib_id, "movie", "OpenRange", None, Some(2024), None,
        Some(120), None, None, None, None, None,
    )
    .unwrap();
    let mf = sf_db::queries::media_files::create_media_file(
        &conn,
        item.id,
        video_path.to_str().unwrap(),
        "open_range.mp4",
        500,
        Some("mp4"),
        Some("h264"),
        Some("aac"),
        Some(1920),
        Some(1080),
        None,
        false,
        None,
        "source",
        "C",
        Some(60.0),
    )
    .unwrap();
    let mf_id_str = mf.id.to_string();

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{addr}/api/stream/{mf_id_str}/direct"))
        .header("Range", "bytes=400-")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 206);
    let body = resp.bytes().await.unwrap();
    assert_eq!(body.len(), 100); // 500 - 400 = 100 bytes
}

#[tokio::test]
async fn direct_stream_not_found() {
    let (_h, addr) = TestHarness::with_server().await;

    let resp = reqwest::get(format!(
        "http://{addr}/api/stream/00000000-0000-0000-0000-000000000001/direct"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn direct_stream_invalid_mf_id() {
    let (_h, addr) = TestHarness::with_server().await;

    let resp = reqwest::get(format!(
        "http://{addr}/api/stream/not-a-uuid/direct"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn hls_playlist_missing_media_file() {
    let (_h, addr) = TestHarness::with_server().await;

    let resp = reqwest::get(format!(
        "http://{addr}/api/stream/00000000-0000-0000-0000-000000000001/index.m3u8"
    ))
    .await
    .unwrap();
    // Should return 404 since media file doesn't exist.
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn hls_segment_invalid_id() {
    let (_h, addr) = TestHarness::with_server().await;

    let resp = reqwest::get(format!(
        "http://{addr}/api/stream/not-a-uuid/segment_0.m4s"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn hls_segment_traversal_blocked() {
    let (_h, addr) = TestHarness::with_server().await;

    let resp = reqwest::get(format!(
        "http://{addr}/api/stream/00000000-0000-0000-0000-000000000001/..%2F..%2Fetc%2Fpasswd"
    ))
    .await
    .unwrap();
    // After URL decoding: ../../../etc/passwd — should be blocked by traversal check.
    // The status depends on how axum handles the path; it may be 404 or 422.
    let status = resp.status().as_u16();
    assert!(status == 404 || status == 422 || status == 400);
}

#[tokio::test]
async fn direct_stream_mkv_content_type() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();

    let dir = tempfile::tempdir().unwrap();
    let video_path = dir.path().join("test.mkv");
    std::fs::write(&video_path, vec![0u8; 100]).unwrap();

    let conn = h.conn();
    let item = sf_db::queries::items::create_item(
        &conn, lib_id, "movie", "MkvTest", None, Some(2024), None,
        Some(120), None, None, None, None, None,
    )
    .unwrap();
    let mf = sf_db::queries::media_files::create_media_file(
        &conn,
        item.id,
        video_path.to_str().unwrap(),
        "test.mkv",
        100,
        Some("mkv"),
        Some("hevc"),
        Some("aac"),
        Some(1920),
        Some(1080),
        None,
        false,
        None,
        "source",
        "C",
        Some(60.0),
    )
    .unwrap();
    let mf_id_str = mf.id.to_string();

    let resp = reqwest::get(format!(
        "http://{addr}/api/stream/{mf_id_str}/direct"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap().to_str().unwrap(),
        "video/x-matroska"
    );
}
