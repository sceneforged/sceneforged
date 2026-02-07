//! Integration tests for configuration management routes.

mod common;

use common::TestHarness;

// ---------------------------------------------------------------------------
// Conversion config
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_conversion_config() {
    let (_h, addr) = TestHarness::with_server().await;
    let resp = reqwest::get(format!("http://{addr}/api/config/conversion"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    // Default config should have video_crf.
    assert!(json["video_crf"].is_number());
}

#[tokio::test]
async fn put_conversion_config() {
    let (_h, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();

    // Get current config first.
    let resp = client
        .get(format!("http://{addr}/api/config/conversion"))
        .send()
        .await
        .unwrap();
    let mut config: serde_json::Value = resp.json().await.unwrap();
    config["video_crf"] = serde_json::json!(23);

    let resp = client
        .put(format!("http://{addr}/api/config/conversion"))
        .json(&config)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["video_crf"], 23);
}

// ---------------------------------------------------------------------------
// Arrs config
// ---------------------------------------------------------------------------

#[tokio::test]
async fn arrs_crud() {
    let (_h, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();

    // Initially empty.
    let resp = client
        .get(format!("http://{addr}/api/config/arrs"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let arrs: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(arrs.is_empty());

    // Create an arr.
    let resp = client
        .post(format!("http://{addr}/api/config/arrs"))
        .json(&serde_json::json!({
            "name": "radarr",
            "type": "radarr",
            "url": "http://localhost:7878",
            "api_key": "test-key",
            "enabled": true,
            "auto_rescan": true,
            "auto_rename": false,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    // List should now have one.
    let resp = client
        .get(format!("http://{addr}/api/config/arrs"))
        .send()
        .await
        .unwrap();
    let arrs: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(arrs.len(), 1);
    assert_eq!(arrs[0]["name"], "radarr");

    // Update the arr.
    let resp = client
        .put(format!("http://{addr}/api/config/arrs/radarr"))
        .json(&serde_json::json!({
            "name": "radarr",
            "type": "radarr",
            "url": "http://localhost:8888",
            "api_key": "new-key",
            "enabled": false,
            "auto_rescan": false,
            "auto_rename": true,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Delete the arr.
    let resp = client
        .delete(format!("http://{addr}/api/config/arrs/radarr"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Should be empty again.
    let resp = client
        .get(format!("http://{addr}/api/config/arrs"))
        .send()
        .await
        .unwrap();
    let arrs: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(arrs.is_empty());
}

#[tokio::test]
async fn arr_duplicate_blocked() {
    let (_h, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();

    let arr = serde_json::json!({
        "name": "dup",
        "type": "radarr",
        "url": "http://localhost:7878",
        "api_key": "k",
        "enabled": true,
        "auto_rescan": false,
        "auto_rename": false,
    });

    client
        .post(format!("http://{addr}/api/config/arrs"))
        .json(&arr)
        .send()
        .await
        .unwrap();

    let resp = client
        .post(format!("http://{addr}/api/config/arrs"))
        .json(&arr)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
}

// ---------------------------------------------------------------------------
// Jellyfins config
// ---------------------------------------------------------------------------

#[tokio::test]
async fn jellyfins_crud() {
    let (_h, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();

    // Create.
    let resp = client
        .post(format!("http://{addr}/api/config/jellyfins"))
        .json(&serde_json::json!({
            "name": "main",
            "url": "http://localhost:8096",
            "api_key": "jf-key",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    // List.
    let resp = client
        .get(format!("http://{addr}/api/config/jellyfins"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let jfs: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(jfs.len(), 1);

    // Update.
    let resp = client
        .put(format!("http://{addr}/api/config/jellyfins/main"))
        .json(&serde_json::json!({
            "name": "main",
            "url": "http://localhost:9999",
            "api_key": "new-jf-key",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Delete.
    let resp = client
        .delete(format!("http://{addr}/api/config/jellyfins/main"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
}

// ---------------------------------------------------------------------------
// Config reload and validate
// ---------------------------------------------------------------------------

#[tokio::test]
async fn reload_config() {
    let (_h, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/config/reload"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["status"], "reloaded");
}

#[tokio::test]
async fn validate_config() {
    let (_h, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/config/validate"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json["valid"].is_boolean());
    assert!(json["warnings"].is_array());
}

// ---------------------------------------------------------------------------
// Directory browser
// ---------------------------------------------------------------------------

#[tokio::test]
async fn browse_directory() {
    let (_h, addr) = TestHarness::with_server().await;
    let resp = reqwest::get(format!("http://{addr}/api/config/browse?path=/tmp"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json["entries"].is_array());
}

#[tokio::test]
async fn browse_invalid_path() {
    let (_h, addr) = TestHarness::with_server().await;
    let resp = reqwest::get(format!(
        "http://{addr}/api/config/browse?path=/nonexistent_path_12345"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 400);
}

// ---------------------------------------------------------------------------
// Test arr connection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_arr_connection_not_found() {
    let (_h, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/config/arrs/nonexistent/test"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_arr_connection_failure() {
    let (_h, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();

    // Create an arr with unreachable URL.
    client
        .post(format!("http://{addr}/api/config/arrs"))
        .json(&serde_json::json!({
            "name": "broken",
            "type": "radarr",
            "url": "http://127.0.0.1:1",
            "api_key": "k",
            "enabled": true,
            "auto_rescan": false,
            "auto_rename": false,
        }))
        .send()
        .await
        .unwrap();

    let resp = client
        .post(format!("http://{addr}/api/config/arrs/broken/test"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["success"], false);
}
