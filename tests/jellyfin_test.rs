//! Integration tests for Jellyfin system and user endpoints.

mod common;

use common::TestHarness;

// ---------------------------------------------------------------------------
// System info
// ---------------------------------------------------------------------------

#[tokio::test]
async fn system_info_public() {
    let (_h, addr) = TestHarness::with_server().await;
    let resp = reqwest::get(format!("http://{addr}/System/Info/Public"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["ServerName"], "SceneForged");
    assert!(json["Version"].is_string());
    assert_eq!(json["Id"], "sceneforged-server");
    assert_eq!(json["ProductName"], "SceneForged");
    assert_eq!(json["StartupWizardCompleted"], true);
}

#[tokio::test]
async fn system_info_authenticated() {
    let (_h, addr) = TestHarness::with_server().await;
    let resp = reqwest::get(format!("http://{addr}/System/Info"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["ServerName"], "SceneForged");
}

// ---------------------------------------------------------------------------
// Public users
// ---------------------------------------------------------------------------

#[tokio::test]
async fn public_users_excludes_anonymous() {
    let (h, addr) = TestHarness::with_server().await;
    h.create_user("testjf", "password");

    let resp = reqwest::get(format!("http://{addr}/Users/Public"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let users: Vec<serde_json::Value> = resp.json().await.unwrap();

    // Should not include "anonymous".
    let names: Vec<&str> = users.iter().map(|u| u["Name"].as_str().unwrap()).collect();
    assert!(!names.contains(&"anonymous"));
    assert!(names.contains(&"testjf"));

    // Each user should have Jellyfin PascalCase fields.
    for user in &users {
        assert!(user["Id"].is_string());
        assert!(user["Name"].is_string());
        assert_eq!(user["ServerId"], "sceneforged-server");
        assert_eq!(user["HasPassword"], true);
    }
}

// ---------------------------------------------------------------------------
// Authenticate by name
// ---------------------------------------------------------------------------

#[tokio::test]
async fn authenticate_by_name_success() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (h, addr) = TestHarness::with_server_config(config).await;
    h.create_user("jfuser", "jfpass");

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/Users/AuthenticateByName"))
        .header(
            "Authorization",
            "MediaBrowser Client=\"Test\", Device=\"Test\", DeviceId=\"123\", Version=\"1.0\"",
        )
        .json(&serde_json::json!({
            "Username": "jfuser",
            "Pw": "jfpass",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json["AccessToken"].is_string());
    assert_eq!(json["User"]["Name"], "jfuser");
    assert_eq!(json["ServerId"], "sceneforged-server");
}

#[tokio::test]
async fn authenticate_by_name_bad_credentials() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (h, addr) = TestHarness::with_server_config(config).await;
    h.create_user("jfbad", "correctpw");

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/Users/AuthenticateByName"))
        .json(&serde_json::json!({
            "Username": "jfbad",
            "Pw": "wrongpw",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

// ---------------------------------------------------------------------------
// Get user by ID
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_user_by_id() {
    let (h, addr) = TestHarness::with_server().await;
    let (_, user_id_str) = h.create_user("getme", "pw");

    let resp = reqwest::get(format!("http://{addr}/Users/{user_id_str}"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["Name"], "getme");
    assert_eq!(json["Id"], user_id_str);
}

#[tokio::test]
async fn get_user_not_found() {
    let (_h, addr) = TestHarness::with_server().await;
    let resp = reqwest::get(format!(
        "http://{addr}/Users/00000000-0000-0000-0000-999999999999"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 404);
}
