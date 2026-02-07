//! Integration tests for user management routes.

mod common;

use common::TestHarness;

#[tokio::test]
async fn create_user() {
    let (_h, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://{addr}/api/admin/users"))
        .json(&serde_json::json!({
            "username": "alice",
            "password": "secret123",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["username"], "alice");
    assert_eq!(json["role"], "user");
    assert!(json["id"].is_string());
}

#[tokio::test]
async fn create_user_with_role() {
    let (_h, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://{addr}/api/admin/users"))
        .json(&serde_json::json!({
            "username": "bob",
            "password": "pw",
            "role": "admin",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["role"], "admin");
}

#[tokio::test]
async fn list_users() {
    let (h, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();

    // The anonymous user is seeded by V4 migration.
    // Create another user.
    h.create_user("testuser", "password");

    let resp = client
        .get(format!("http://{addr}/api/admin/users"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let users: Vec<serde_json::Value> = resp.json().await.unwrap();
    // Should have anonymous + testuser.
    assert!(users.len() >= 2);
    let names: Vec<&str> = users.iter().map(|u| u["username"].as_str().unwrap()).collect();
    assert!(names.contains(&"testuser"));
}

#[tokio::test]
async fn update_user_role() {
    let (h, addr) = TestHarness::with_server().await;
    let (user_id, user_id_str) = h.create_user("updater", "pass");
    let _ = user_id;

    let client = reqwest::Client::new();
    let resp = client
        .put(format!("http://{addr}/api/admin/users/{user_id_str}"))
        .json(&serde_json::json!({"role": "admin"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Verify the role changed.
    let conn = h.conn();
    let user = sf_db::queries::users::get_user_by_id(&conn, user_id).unwrap().unwrap();
    assert_eq!(user.role, "admin");
}

#[tokio::test]
async fn update_user_password() {
    let (h, addr) = TestHarness::with_server().await;
    let (user_id, user_id_str) = h.create_user("pwchg", "old");

    let client = reqwest::Client::new();
    let resp = client
        .put(format!("http://{addr}/api/admin/users/{user_id_str}"))
        .json(&serde_json::json!({"password": "newpassword"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Verify the password changed.
    let conn = h.conn();
    let user = sf_db::queries::users::get_user_by_id(&conn, user_id).unwrap().unwrap();
    assert!(bcrypt::verify("newpassword", &user.password_hash).unwrap());
}

#[tokio::test]
async fn delete_user() {
    let (h, addr) = TestHarness::with_server().await;
    let (user_id, user_id_str) = h.create_user("deleteme", "pass");

    let client = reqwest::Client::new();
    let resp = client
        .delete(format!("http://{addr}/api/admin/users/{user_id_str}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Verify the user is gone.
    let conn = h.conn();
    assert!(sf_db::queries::users::get_user_by_id(&conn, user_id).unwrap().is_none());
}
