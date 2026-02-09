//! Integration tests for the invitation system.

mod common;

use common::TestHarness;

#[tokio::test]
async fn create_invitation() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (h, addr) = TestHarness::with_server_config(config).await;
    let (admin_id, _) = h.create_admin_user("admin", "pass");
    let token = h.auth_token(admin_id);

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/admin/invitations"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({
            "role": "user",
            "expires_in_days": 7,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json["code"].is_string());
    assert_eq!(json["code"].as_str().unwrap().len(), 8);
    assert_eq!(json["role"], "user");
}

#[tokio::test]
async fn list_invitations() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (h, addr) = TestHarness::with_server_config(config).await;
    let (admin_id, _) = h.create_admin_user("admin", "pass");
    let token = h.auth_token(admin_id);

    let client = reqwest::Client::new();
    // Create two invitations.
    for _ in 0..2 {
        client
            .post(format!("http://{addr}/api/admin/invitations"))
            .header("Authorization", format!("Bearer {token}"))
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap();
    }

    let resp = client
        .get(format!("http://{addr}/api/admin/invitations"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let invitations: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(invitations.len(), 2);
}

#[tokio::test]
async fn delete_invitation() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (h, addr) = TestHarness::with_server_config(config).await;
    let (admin_id, _) = h.create_admin_user("admin", "pass");
    let token = h.auth_token(admin_id);

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/admin/invitations"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let json: serde_json::Value = resp.json().await.unwrap();
    let inv_id = json["id"].as_str().unwrap();

    let resp = client
        .delete(format!("http://{addr}/api/admin/invitations/{inv_id}"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Verify it's gone.
    let resp = client
        .get(format!("http://{addr}/api/admin/invitations"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    let invitations: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(invitations.is_empty());
}

#[tokio::test]
async fn register_with_valid_code() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (h, addr) = TestHarness::with_server_config(config).await;
    let (admin_id, _) = h.create_admin_user("admin", "pass");
    let token = h.auth_token(admin_id);

    let client = reqwest::Client::new();
    // Create an invitation.
    let resp = client
        .post(format!("http://{addr}/api/admin/invitations"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    let inv: serde_json::Value = resp.json().await.unwrap();
    let code = inv["code"].as_str().unwrap();

    // Register without auth using the code.
    let resp = client
        .post(format!("http://{addr}/api/auth/register"))
        .json(&serde_json::json!({
            "code": code,
            "username": "newuser",
            "password": "newpass123",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["success"], true);
    assert!(json["token"].is_string());
}

#[tokio::test]
async fn register_with_invalid_code() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (_h, addr) = TestHarness::with_server_config(config).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/auth/register"))
        .json(&serde_json::json!({
            "code": "BADCODE1",
            "username": "newuser",
            "password": "newpass123",
        }))
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    assert!(status == 400 || status == 422, "expected 400 or 422, got {status}");
}

#[tokio::test]
async fn register_with_used_code() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (h, addr) = TestHarness::with_server_config(config).await;
    let (admin_id, _) = h.create_admin_user("admin", "pass");
    let token = h.auth_token(admin_id);

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/admin/invitations"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    let inv: serde_json::Value = resp.json().await.unwrap();
    let code = inv["code"].as_str().unwrap();

    // First registration succeeds.
    let resp = client
        .post(format!("http://{addr}/api/auth/register"))
        .json(&serde_json::json!({
            "code": code,
            "username": "user1",
            "password": "pass1",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    // Second registration with same code fails.
    let resp = client
        .post(format!("http://{addr}/api/auth/register"))
        .json(&serde_json::json!({
            "code": code,
            "username": "user2",
            "password": "pass2",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn register_with_expired_code() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (h, addr) = TestHarness::with_server_config(config).await;
    let (admin_id, _) = h.create_admin_user("admin", "pass");

    // Create an already-expired invitation via DB helper.
    let (_inv_id, code) = h.create_invitation("user", -1, admin_id);

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/auth/register"))
        .json(&serde_json::json!({
            "code": code,
            "username": "newuser",
            "password": "newpass",
        }))
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    assert!(status == 400 || status == 422, "expected 400 or 422, got {status}");
}
