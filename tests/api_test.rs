//! API integration tests.
//!
//! Tests HTTP API endpoints against a [`TestHarness`] server running on a
//! random port with an in-memory SQLite database.

mod common;

use common::TestHarness;

// ---------------------------------------------------------------------------
// Health check
// ---------------------------------------------------------------------------

#[tokio::test]
async fn health_check_returns_200() {
    let (_harness, addr) = TestHarness::with_server().await;
    let url = format!("http://{addr}/health");

    let resp = reqwest::get(&url).await.expect("request failed");
    assert_eq!(resp.status(), 200);

    let body = resp.text().await.unwrap();
    assert_eq!(body, "ok");
}

// ---------------------------------------------------------------------------
// Auth flow (auth disabled by default)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn auth_status_when_disabled() {
    let (_harness, addr) = TestHarness::with_server().await;
    let url = format!("http://{addr}/api/auth/status");

    let resp = reqwest::get(&url).await.unwrap();
    assert_eq!(resp.status(), 200);

    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["auth_enabled"], false);
    assert_eq!(json["authenticated"], true);
}

#[tokio::test]
async fn login_when_auth_disabled() {
    let (_harness, addr) = TestHarness::with_server().await;
    let url = format!("http://{addr}/api/auth/login");
    let client = reqwest::Client::new();

    let resp = client
        .post(&url)
        .json(&serde_json::json!({"username": "admin", "password": "pass"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["message"], "Auth disabled");
}

#[tokio::test]
async fn logout_returns_200() {
    let (_harness, addr) = TestHarness::with_server().await;
    let url = format!("http://{addr}/api/auth/logout");
    let client = reqwest::Client::new();

    let resp = client.post(&url).send().await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn auth_login_with_credentials() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    config.auth.username = Some("admin".into());
    config.auth.password_hash = Some("secret".into());

    let (_harness, addr) = TestHarness::with_server_config(config).await;
    let client = reqwest::Client::new();
    let base = format!("http://{addr}");

    // Login with correct credentials.
    let resp = client
        .post(format!("{base}/api/auth/login"))
        .json(&serde_json::json!({"username": "admin", "password": "secret"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["success"], true);
    let token = json["token"].as_str().unwrap();

    // Auth status with bearer token should show authenticated.
    let resp = client
        .get(format!("{base}/api/auth/status"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["authenticated"], true);
    assert_eq!(json["auth_enabled"], true);

    // Logout.
    let resp = client
        .post(format!("{base}/api/auth/logout"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Auth status without token should show unauthenticated.
    let resp = client
        .get(format!("{base}/api/auth/status"))
        .send()
        .await
        .unwrap();
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["authenticated"], false);
}

#[tokio::test]
async fn auth_login_with_bad_credentials() {
    let mut config = sf_core::config::Config::default();
    config.auth.enabled = true;
    config.auth.username = Some("admin".into());
    config.auth.password_hash = Some("secret".into());

    let (_harness, addr) = TestHarness::with_server_config(config).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://{addr}/api/auth/login"))
        .json(&serde_json::json!({"username": "admin", "password": "wrong"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

// ---------------------------------------------------------------------------
// Library CRUD
// ---------------------------------------------------------------------------

#[tokio::test]
async fn library_crud() {
    let (_harness, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{addr}/api");

    // List libraries (initially empty).
    let resp = client.get(format!("{base}/libraries")).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let libs: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(libs.is_empty());

    // Create a library.
    let resp = client
        .post(format!("{base}/libraries"))
        .json(&serde_json::json!({
            "name": "Movies",
            "media_type": "movies",
            "paths": ["/media/movies"],
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let lib: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(lib["name"], "Movies");
    assert_eq!(lib["media_type"], "movies");
    let lib_id = lib["id"].as_str().unwrap().to_string();

    // Get the library by ID.
    let resp = client
        .get(format!("{base}/libraries/{lib_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let fetched: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(fetched["name"], "Movies");

    // List libraries (should have one now).
    let resp = client.get(format!("{base}/libraries")).send().await.unwrap();
    let libs: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(libs.len(), 1);

    // Delete the library.
    let resp = client
        .delete(format!("{base}/libraries/{lib_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Verify it is gone.
    let resp = client
        .get(format!("{base}/libraries/{lib_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn create_library_validates_name() {
    let (_harness, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://{addr}/api/libraries"))
        .json(&serde_json::json!({
            "name": "",
            "media_type": "movies",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

// ---------------------------------------------------------------------------
// Job submission and retrieval
// ---------------------------------------------------------------------------

#[tokio::test]
async fn job_submit_and_get() {
    let (_harness, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{addr}/api");

    // Submit a job.
    let resp = client
        .post(format!("{base}/jobs/submit"))
        .json(&serde_json::json!({
            "file_path": "/media/movies/test.mkv",
            "priority": 5,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let job: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(job["status"], "queued");
    assert_eq!(job["file_name"], "test.mkv");
    assert_eq!(job["priority"], 5);
    let job_id = job["id"].as_str().unwrap().to_string();

    // Get the job by ID.
    let resp = client
        .get(format!("{base}/jobs/{job_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let fetched: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(fetched["id"], job_id);
    assert_eq!(fetched["status"], "queued");

    // List all jobs.
    let resp = client.get(format!("{base}/jobs")).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let jobs: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(jobs.len(), 1);

    // List jobs filtered by status.
    let resp = client
        .get(format!("{base}/jobs?status=queued"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let jobs: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(jobs.len(), 1);

    let resp = client
        .get(format!("{base}/jobs?status=completed"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let jobs: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(jobs.is_empty());
}

#[tokio::test]
async fn job_submit_validates_file_path() {
    let (_harness, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://{addr}/api/jobs/submit"))
        .json(&serde_json::json!({"file_path": ""}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn get_nonexistent_job_returns_404() {
    let (_harness, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!(
            "http://{addr}/api/jobs/00000000-0000-0000-0000-000000000000"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// ---------------------------------------------------------------------------
// Rules get/update
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rules_get_and_put() {
    let (_harness, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{addr}/api/config");

    // Get rules (initially empty).
    let resp = client.get(format!("{base}/rules")).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let rules: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(rules.is_empty());

    // Put a new set of rules. Build JSON through sf_rules helpers to
    // ensure the format is correct.
    let rule = sf_rules::Rule {
        id: sf_core::RuleId::new(),
        name: "test_rule".into(),
        enabled: true,
        priority: 10,
        expr: sf_rules::Expr::Condition(sf_rules::Condition::Container(vec![
            sf_core::Container::Mkv,
        ])),
        actions: vec![],
    };
    let rules_json = sf_rules::serialize_rules(&[rule]).unwrap();

    let resp = client
        .put(format!("{base}/rules"))
        .header("Content-Type", "application/json")
        .body(rules_json)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let returned: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0]["name"], "test_rule");

    // Get rules again.
    let resp = client.get(format!("{base}/rules")).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let rules: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(rules.len(), 1);
}

// ---------------------------------------------------------------------------
// Dashboard stats
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dashboard_stats() {
    let (_harness, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{addr}/api/admin");

    let resp = client.get(format!("{base}/dashboard")).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json["jobs"]["total"].is_number());
    assert!(json["jobs"]["queued"].is_number());
    assert!(json["jobs"]["processing"].is_number());
    assert!(json["event_bus"]["recent_events"].is_number());
}

#[tokio::test]
async fn dashboard_stats_reflect_jobs() {
    let (harness, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{addr}");

    // Submit two jobs.
    for name in &["a.mkv", "b.mkv"] {
        client
            .post(format!("{base}/api/jobs/submit"))
            .json(&serde_json::json!({"file_path": format!("/media/{name}")}))
            .send()
            .await
            .unwrap();
    }

    // Dashboard should now show 2 total and 2 queued.
    let resp = client
        .get(format!("{base}/api/admin/dashboard"))
        .send()
        .await
        .unwrap();
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["jobs"]["total"], 2);
    assert_eq!(json["jobs"]["queued"], 2);

    // Dequeue one job directly via db to move it to processing.
    {
        let conn = harness.conn();
        sf_db::queries::jobs::dequeue_next(&conn, "test-worker").unwrap();
    }

    let resp = client
        .get(format!("{base}/api/admin/dashboard"))
        .send()
        .await
        .unwrap();
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["jobs"]["queued"], 1);
    assert_eq!(json["jobs"]["processing"], 1);
}

#[tokio::test]
async fn tools_endpoint() {
    let (_harness, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://{addr}/api/admin/tools"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json.is_array());
}

// ---------------------------------------------------------------------------
// Metrics endpoint
// ---------------------------------------------------------------------------

#[tokio::test]
async fn metrics_endpoint_returns_200() {
    let (_harness, addr) = TestHarness::with_server().await;

    let resp = reqwest::get(format!("http://{addr}/metrics"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}
