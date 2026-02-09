//! Integration tests for admin role enforcement middleware.

mod common;

use common::TestHarness;

#[tokio::test]
async fn non_admin_rejected_from_admin_routes() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (h, addr) = TestHarness::with_server_config(config).await;
    let (user_id, _) = h.create_user("regular", "pass");
    let token = h.auth_token(user_id);

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{addr}/api/admin/users"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn admin_allowed_on_admin_routes() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (h, addr) = TestHarness::with_server_config(config).await;
    let (admin_id, _) = h.create_admin_user("admin", "pass");
    let token = h.auth_token(admin_id);

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{addr}/api/admin/users"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn non_admin_cannot_create_user() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (h, addr) = TestHarness::with_server_config(config).await;
    let (user_id, _) = h.create_user("regular", "pass");
    let token = h.auth_token(user_id);

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/admin/users"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({
            "username": "hacker",
            "password": "pwned",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn non_admin_cannot_manage_invitations() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    let (h, addr) = TestHarness::with_server_config(config).await;
    let (user_id, _) = h.create_user("regular", "pass");
    let token = h.auth_token(user_id);

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/admin/invitations"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}
