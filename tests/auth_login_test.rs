//! Integration tests for auth login/logout routes.

mod common;

use common::TestHarness;

#[tokio::test]
async fn login_success() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (h, addr) = TestHarness::with_server_config(config).await;
    h.create_user("testuser", "correct-password");

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/auth/login"))
        .json(&serde_json::json!({
            "username": "testuser",
            "password": "correct-password",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["success"], true);
    assert!(json["token"].is_string());
}

#[tokio::test]
async fn login_invalid_password() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (h, addr) = TestHarness::with_server_config(config).await;
    h.create_user("testuser", "correct-password");

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/auth/login"))
        .json(&serde_json::json!({
            "username": "testuser",
            "password": "wrong-password",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn login_nonexistent_user() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (_h, addr) = TestHarness::with_server_config(config).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/auth/login"))
        .json(&serde_json::json!({
            "username": "nobody",
            "password": "anything",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn login_auth_disabled() {
    let (_h, addr) = TestHarness::with_server().await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/auth/login"))
        .json(&serde_json::json!({
            "username": "anyone",
            "password": "anything",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["success"], true);
}

#[tokio::test]
async fn logout_invalidates_token() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (h, addr) = TestHarness::with_server_config(config).await;
    h.create_user("logoutuser", "pass123");

    let client = reqwest::Client::new();

    // Login to get a token.
    let resp = client
        .post(format!("http://{addr}/api/auth/login"))
        .json(&serde_json::json!({
            "username": "logoutuser",
            "password": "pass123",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    let token = json["token"].as_str().unwrap().to_string();

    // Use the token (should work).
    let resp = client
        .get(format!("http://{addr}/api/libraries"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Logout.
    let resp = client
        .post(format!("http://{addr}/api/auth/logout"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Token should now be invalid.
    let resp = client
        .get(format!("http://{addr}/api/libraries"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}
