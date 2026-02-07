//! Integration tests for Jellyfin playstate reporting endpoints.

mod common;

use common::TestHarness;

#[tokio::test]
async fn report_playing_start() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, _, item_id_str, _) = h.create_item_with_media(lib_id, "Play Movie", "movie");

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/Sessions/Playing"))
        .json(&serde_json::json!({
            "ItemId": item_id_str,
            "PositionTicks": 0,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn report_progress() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, _, item_id_str, _) = h.create_item_with_media(lib_id, "Prog Movie", "movie");

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/Sessions/Playing/Progress"))
        .json(&serde_json::json!({
            "ItemId": item_id_str,
            "PositionTicks": 600_000_000_i64,  // 60 seconds
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn report_stopped() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (item_id, _, item_id_str, _) = h.create_item_with_media(lib_id, "Stop Movie", "movie");

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/Sessions/Playing/Stopped"))
        .json(&serde_json::json!({
            "ItemId": item_id_str,
            "PositionTicks": 1_200_000_000_i64,  // 120 seconds
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Verify playback state persisted to DB.
    let conn = h.conn();
    let anon_uid: sf_core::UserId = "00000000-0000-0000-0000-000000000000".parse().unwrap();
    let pb = sf_db::queries::playback::get_playback(&conn, anon_uid, item_id)
        .unwrap()
        .unwrap();
    assert!((pb.position_secs - 120.0).abs() < 0.1);
}

#[tokio::test]
async fn playstate_full_lifecycle() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (item_id, _, item_id_str, _) = h.create_item_with_media(lib_id, "Life Movie", "movie");

    let client = reqwest::Client::new();

    // Start playing.
    client
        .post(format!("http://{addr}/Sessions/Playing"))
        .json(&serde_json::json!({"ItemId": item_id_str, "PositionTicks": 0}))
        .send()
        .await
        .unwrap();

    // Report progress at 5 minutes.
    client
        .post(format!("http://{addr}/Sessions/Playing/Progress"))
        .json(&serde_json::json!({
            "ItemId": item_id_str,
            "PositionTicks": 3_000_000_000_i64,  // 300 seconds
        }))
        .send()
        .await
        .unwrap();

    // Stop at 10 minutes.
    client
        .post(format!("http://{addr}/Sessions/Playing/Stopped"))
        .json(&serde_json::json!({
            "ItemId": item_id_str,
            "PositionTicks": 6_000_000_000_i64,  // 600 seconds
        }))
        .send()
        .await
        .unwrap();

    // Verify final position.
    let conn = h.conn();
    let anon_uid: sf_core::UserId = "00000000-0000-0000-0000-000000000000".parse().unwrap();
    let pb = sf_db::queries::playback::get_playback(&conn, anon_uid, item_id)
        .unwrap()
        .unwrap();
    assert!((pb.position_secs - 600.0).abs() < 0.1);
    // play_count should be 3 (one for each upsert).
    assert_eq!(pb.play_count, 3);
}
