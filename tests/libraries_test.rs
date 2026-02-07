//! Extended integration tests for library routes.

mod common;

use common::TestHarness;

#[tokio::test]
async fn list_library_items() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, lib_id_str) = h.create_library();
    h.create_item(lib_id);
    h.create_item_with_media(lib_id, "Item 2", "movie");

    let resp = reqwest::get(format!(
        "http://{addr}/api/libraries/{lib_id_str}/items"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let items: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(items.len(), 2);
}

#[tokio::test]
async fn list_library_items_pagination() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, lib_id_str) = h.create_library();
    for i in 0..5 {
        h.create_item_with_media(lib_id, &format!("Movie {i}"), "movie");
    }

    let resp = reqwest::get(format!(
        "http://{addr}/api/libraries/{lib_id_str}/items?offset=0&limit=3"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let items: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(items.len(), 3);

    let resp = reqwest::get(format!(
        "http://{addr}/api/libraries/{lib_id_str}/items?offset=3&limit=3"
    ))
    .await
    .unwrap();
    let items: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(items.len(), 2);
}

#[tokio::test]
async fn list_library_recent() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, lib_id_str) = h.create_library();
    // Items created now should be within the last 7 days.
    h.create_item(lib_id);

    let resp = reqwest::get(format!(
        "http://{addr}/api/libraries/{lib_id_str}/recent"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let items: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(items.len(), 1);
}

#[tokio::test]
async fn list_library_items_not_found() {
    let (_h, addr) = TestHarness::with_server().await;
    let resp = reqwest::get(format!(
        "http://{addr}/api/libraries/00000000-0000-0000-0000-000000000001/items"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn scan_library() {
    let (h, addr) = TestHarness::with_server().await;
    let (_, lib_id_str) = h.create_library();

    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "http://{addr}/api/libraries/{lib_id_str}/scan"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 202);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["status"], "scan_queued");
}

#[tokio::test]
async fn scan_library_not_found() {
    let (_h, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "http://{addr}/api/libraries/00000000-0000-0000-0000-000000000001/scan"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn admin_stats() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    h.create_item(lib_id);

    let resp = reqwest::get(format!("http://{addr}/api/admin/stats"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json["total_items"].as_i64().unwrap() >= 1);
    assert!(json["total_files"].as_i64().unwrap() >= 1);
    assert!(json["storage_bytes"].as_i64().unwrap() > 0);
    assert!(json["items_by_profile"].is_object());
}
