//! Integration tests for playback and favorites routes.

mod common;

use common::TestHarness;

#[tokio::test]
async fn update_progress_creates_playback() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, item_id_str) = h.create_item(lib_id);

    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "http://{addr}/api/playback/{item_id_str}/progress"
        ))
        .json(&serde_json::json!({"position_secs": 120.5}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["item_id"], item_id_str);
    assert!((json["position_secs"].as_f64().unwrap() - 120.5).abs() < 0.1);
    assert_eq!(json["completed"], false);
    assert_eq!(json["play_count"], 1);
}

#[tokio::test]
async fn get_playback_state() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, item_id_str) = h.create_item(lib_id);

    let client = reqwest::Client::new();
    // Create playback state.
    client
        .post(format!(
            "http://{addr}/api/playback/{item_id_str}/progress"
        ))
        .json(&serde_json::json!({"position_secs": 300.0}))
        .send()
        .await
        .unwrap();

    // Get playback state.
    let resp = client
        .get(format!("http://{addr}/api/playback/{item_id_str}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!((json["position_secs"].as_f64().unwrap() - 300.0).abs() < 0.1);
}

#[tokio::test]
async fn get_playback_not_found() {
    let (_h, addr) = TestHarness::with_server().await;
    let resp = reqwest::get(format!(
        "http://{addr}/api/playback/00000000-0000-0000-0000-000000000001"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn mark_played() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, item_id_str) = h.create_item(lib_id);

    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "http://{addr}/api/playback/{item_id_str}/played"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["completed"], true);
}

#[tokio::test]
async fn mark_unplayed() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, item_id_str) = h.create_item(lib_id);

    let client = reqwest::Client::new();
    // First mark as played.
    client
        .post(format!(
            "http://{addr}/api/playback/{item_id_str}/played"
        ))
        .send()
        .await
        .unwrap();

    // Then mark unplayed.
    let resp = client
        .post(format!(
            "http://{addr}/api/playback/{item_id_str}/unplayed"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Playback should now be gone.
    let resp = client
        .get(format!("http://{addr}/api/playback/{item_id_str}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn continue_watching() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, item1_str) = h.create_item(lib_id);
    let (_, _, item2_str, _) = h.create_item_with_media(lib_id, "Movie 2", "movie");

    let client = reqwest::Client::new();

    // Update progress on both items.
    client
        .post(format!(
            "http://{addr}/api/playback/{item1_str}/progress"
        ))
        .json(&serde_json::json!({"position_secs": 100.0}))
        .send()
        .await
        .unwrap();
    client
        .post(format!(
            "http://{addr}/api/playback/{item2_str}/progress"
        ))
        .json(&serde_json::json!({"position_secs": 200.0}))
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!("http://{addr}/api/playback/continue"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let entries: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(entries.len(), 2);
    // Each entry should have an "item" object.
    assert!(entries[0]["item"]["name"].is_string());
}

#[tokio::test]
async fn add_and_remove_favorite() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, item_id_str) = h.create_item(lib_id);

    let client = reqwest::Client::new();

    // Add favorite.
    let resp = client
        .post(format!("http://{addr}/api/favorites/{item_id_str}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["item_id"], item_id_str);

    // Remove favorite.
    let resp = client
        .delete(format!("http://{addr}/api/favorites/{item_id_str}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn list_favorites_enriched() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, item_id_str) = h.create_item(lib_id);

    let client = reqwest::Client::new();
    client
        .post(format!("http://{addr}/api/favorites/{item_id_str}"))
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!("http://{addr}/api/favorites"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let entries: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["item"]["name"], "Test Movie");
}

#[tokio::test]
async fn get_user_data_combined() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, item_id_str) = h.create_item(lib_id);

    let client = reqwest::Client::new();

    // Add progress and favorite.
    client
        .post(format!(
            "http://{addr}/api/playback/{item_id_str}/progress"
        ))
        .json(&serde_json::json!({"position_secs": 50.0}))
        .send()
        .await
        .unwrap();
    client
        .post(format!("http://{addr}/api/favorites/{item_id_str}"))
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!(
            "http://{addr}/api/playback/{item_id_str}/user-data"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json["playback"].is_object());
    assert_eq!(json["is_favorite"], true);
}
