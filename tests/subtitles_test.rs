//! Integration tests for subtitle track routes.

mod common;

use common::TestHarness;

#[tokio::test]
async fn list_subtitles_empty() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, _, item_id_str, _) = h.create_item_with_media(lib_id, "NoSubs", "movie");

    let resp = reqwest::get(format!(
        "http://{addr}/api/items/{item_id_str}/subtitles"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let tracks: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(tracks.is_empty());
}

#[tokio::test]
async fn list_subtitles_with_tracks() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, mf_id, item_id_str, _) = h.create_item_with_media(lib_id, "SubsMovie", "movie");

    // Insert subtitle tracks.
    let conn = h.conn();
    sf_db::queries::subtitle_tracks::create_subtitle_track(
        &conn, mf_id, 0, "srt", Some("eng"), false, true,
    )
    .unwrap();
    sf_db::queries::subtitle_tracks::create_subtitle_track(
        &conn, mf_id, 1, "ass", Some("jpn"), false, false,
    )
    .unwrap();
    sf_db::queries::subtitle_tracks::create_subtitle_track(
        &conn, mf_id, 2, "srt", Some("eng"), true, false,
    )
    .unwrap();

    let resp = reqwest::get(format!(
        "http://{addr}/api/items/{item_id_str}/subtitles"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let tracks: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(tracks.len(), 3);
    assert_eq!(tracks[0]["codec"], "srt");
    assert_eq!(tracks[0]["language"], "eng");
    assert_eq!(tracks[0]["default_track"], true);
    assert_eq!(tracks[1]["language"], "jpn");
    assert_eq!(tracks[2]["forced"], true);
}

#[tokio::test]
async fn list_subtitles_invalid_item_id() {
    let (_h, addr) = TestHarness::with_server().await;

    let resp = reqwest::get(format!(
        "http://{addr}/api/items/not-a-uuid/subtitles"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn get_subtitle_track_not_found() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, _, _, mf_id_str) = h.create_item_with_media(lib_id, "NoTrack", "movie");

    let resp = reqwest::get(format!(
        "http://{addr}/api/stream/{mf_id_str}/subtitles/0"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn get_subtitle_invalid_mf_id() {
    let (_h, addr) = TestHarness::with_server().await;

    let resp = reqwest::get(format!(
        "http://{addr}/api/stream/not-a-uuid/subtitles/0"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn get_subtitle_missing_media_file() {
    let (_h, addr) = TestHarness::with_server().await;

    let resp = reqwest::get(format!(
        "http://{addr}/api/stream/00000000-0000-0000-0000-000000000001/subtitles/0"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 404);
}
