//! Integration tests for Jellyfin PlaybackInfo endpoint.

mod common;

use common::TestHarness;

#[tokio::test]
async fn playback_info_returns_media_sources() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, _, item_id_str, _) = h.create_item_with_media(lib_id, "Test Movie", "movie");

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/Items/{item_id_str}/PlaybackInfo"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json["PlaySessionId"].is_string());
    let sources = json["MediaSources"].as_array().unwrap();
    assert!(!sources.is_empty());

    let source = &sources[0];
    assert!(source["SupportsDirectStream"].as_bool().unwrap());
    assert!(source["MediaStreams"].is_array());
    let streams = source["MediaStreams"].as_array().unwrap();
    // Should have at least video + audio.
    assert!(streams.len() >= 2);

    // First stream should be video.
    assert_eq!(streams[0]["Type"], "Video");
    assert_eq!(streams[0]["Codec"], "hevc");
}

#[tokio::test]
async fn playback_info_with_subtitles() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, mf_id, item_id_str, _) = h.create_item_with_media(lib_id, "Sub Movie", "movie");

    // Add subtitle tracks.
    h.create_subtitle_track(mf_id, 0, "srt", Some("eng"));
    h.create_subtitle_track(mf_id, 1, "ass", Some("jpn"));

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/Items/{item_id_str}/PlaybackInfo"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    let sources = json["MediaSources"].as_array().unwrap();
    let streams = sources[0]["MediaStreams"].as_array().unwrap();

    // video + audio + 2 subtitle = 4 streams.
    assert_eq!(streams.len(), 4);

    let subs: Vec<&serde_json::Value> = streams
        .iter()
        .filter(|s| s["Type"] == "Subtitle")
        .collect();
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0]["Codec"], "srt");
    assert_eq!(subs[0]["Language"], "eng");
    assert_eq!(subs[1]["Codec"], "ass");
    assert_eq!(subs[1]["Language"], "jpn");
}

#[tokio::test]
async fn playback_info_not_found() {
    let (_h, addr) = TestHarness::with_server().await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "http://{addr}/Items/00000000-0000-0000-0000-000000000001/PlaybackInfo"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn playback_info_invalid_id() {
    let (_h, addr) = TestHarness::with_server().await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/Items/not-a-uuid/PlaybackInfo"))
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    assert!(status == 400 || status == 422, "expected 400 or 422, got {status}");
}
