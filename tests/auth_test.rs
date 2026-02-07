//! Integration tests for auth middleware coverage.
//!
//! Tests the various authentication header formats:
//! MediaBrowser, X-Emby-Token, Bearer, Cookie, and api_key config.

mod common;

use common::TestHarness;

#[tokio::test]
async fn auth_with_mediabrowser_header() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (h, addr) = TestHarness::with_server_config(config).await;
    let (user_id, _) = h.create_user("mbuser", "pw");
    let token = h.auth_token(user_id);

    // MediaBrowser header is validated by the auth middleware on protected routes.
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{addr}/api/libraries"))
        .header(
            "Authorization",
            format!("MediaBrowser Client=\"Test\", Device=\"Test\", DeviceId=\"123\", Version=\"1.0\", Token=\"{token}\""),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn auth_with_emby_prefix() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (h, addr) = TestHarness::with_server_config(config).await;
    let (user_id, _) = h.create_user("embyuser", "pw");
    let token = h.auth_token(user_id);

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{addr}/api/libraries"))
        .header(
            "Authorization",
            format!("Emby Client=\"Test\", Token=\"{token}\""),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn auth_with_x_emby_token() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (h, addr) = TestHarness::with_server_config(config).await;
    let (user_id, _) = h.create_user("xemby", "pw");
    let token = h.auth_token(user_id);

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{addr}/api/libraries"))
        .header("X-Emby-Token", &token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn auth_with_cookie() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (h, addr) = TestHarness::with_server_config(config).await;
    let (user_id, _) = h.create_user("cookieuser", "pw");
    let token = h.auth_token(user_id);

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{addr}/api/auth/status"))
        .header("Cookie", format!("sceneforged_session={token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["authenticated"], true);
}

#[tokio::test]
async fn auth_with_api_key_config() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    config.auth.api_key = Some("my-secret-api-key".into());
    let (_h, addr) = TestHarness::with_server_config(config).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{addr}/api/auth/status"))
        .header("Authorization", "Bearer my-secret-api-key")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["authenticated"], true);
}

#[tokio::test]
async fn auth_invalid_token_rejected() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (_h, addr) = TestHarness::with_server_config(config).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{addr}/api/libraries"))
        .header("Authorization", "Bearer invalid-token-12345")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn unauthenticated_access_blocked() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (_h, addr) = TestHarness::with_server_config(config).await;

    // No auth header at all.
    let resp = reqwest::get(format!("http://{addr}/api/libraries"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn health_accessible_without_auth() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (_h, addr) = TestHarness::with_server_config(config).await;

    let resp = reqwest::get(format!("http://{addr}/health"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn auth_status_accessible_without_token() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (_h, addr) = TestHarness::with_server_config(config).await;

    let resp = reqwest::get(format!("http://{addr}/api/auth/status"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["authenticated"], false);
    assert_eq!(json["auth_enabled"], true);
}
