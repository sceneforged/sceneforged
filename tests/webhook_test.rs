//! Webhook integration tests.
//!
//! Tests webhook processing for Radarr/Sonarr webhooks, including payload
//! parsing, job creation, and signature verification.

mod common;

use common::TestHarness;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Compute an HMAC-SHA256 signature for the given body using the provided secret.
fn compute_signature(secret: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body);
    let result = mac.finalize();
    format!("sha256={}", hex::encode(result.into_bytes()))
}

// ---------------------------------------------------------------------------
// Radarr webhook parsing and job creation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn radarr_webhook_creates_job() {
    let (harness, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();

    // Configure an arr so the webhook handler recognises it.
    {
        let mut arrs = harness.ctx.config_store.arrs.write();
        arrs.push(sf_core::config::ArrConfig {
            name: "radarr".into(),
            arr_type: "radarr".into(),
            url: "http://localhost:7878".into(),
            api_key: "test-key".into(),
            enabled: true,
            auto_rescan: true,
            auto_rename: false,
        });
    }

    // Create a temp file so the webhook's file-existence check passes.
    let tmp_dir = tempfile::tempdir().unwrap();
    let movie_dir = tmp_dir.path().join("Inception (2010)");
    std::fs::create_dir_all(&movie_dir).unwrap();
    let movie_file = movie_dir.join("Inception.2010.mkv");
    std::fs::write(&movie_file, b"fake movie data").unwrap();

    let movie_file_str = movie_file.to_str().unwrap();
    let movie_dir_str = movie_dir.to_str().unwrap();

    let payload = serde_json::json!({
        "eventType": "Download",
        "movie": {
            "title": "Inception",
            "folderPath": movie_dir_str
        },
        "movieFile": {
            "path": movie_file_str,
            "quality": "Bluray-1080p"
        }
    });

    let resp = client
        .post(format!("http://{addr}/webhook/radarr"))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json["job_id"].is_string(), "Expected job_id in response");

    // Verify job was created in the database.
    let conn = harness.conn();
    let jobs = sf_db::queries::jobs::list_jobs(&conn, Some("queued"), 0, 100).unwrap();
    assert_eq!(jobs.len(), 1);
    assert!(jobs[0].file_path.contains("Inception"));
    assert_eq!(jobs[0].source.as_deref(), Some("radarr"));
}

// ---------------------------------------------------------------------------
// Sonarr webhook parsing and job creation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sonarr_webhook_creates_job() {
    let (harness, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();

    {
        let mut arrs = harness.ctx.config_store.arrs.write();
        arrs.push(sf_core::config::ArrConfig {
            name: "sonarr".into(),
            arr_type: "sonarr".into(),
            url: "http://localhost:8989".into(),
            api_key: "test-key".into(),
            enabled: true,
            auto_rescan: true,
            auto_rename: false,
        });
    }

    // Create a temp file so the webhook's file-existence check passes.
    let tmp_dir = tempfile::tempdir().unwrap();
    let tv_dir = tmp_dir.path().join("Breaking Bad");
    std::fs::create_dir_all(&tv_dir).unwrap();
    let episode_file = tv_dir.join("S01E01.mkv");
    std::fs::write(&episode_file, b"fake episode data").unwrap();

    let episode_file_str = episode_file.to_str().unwrap();

    let payload = serde_json::json!({
        "eventType": "Download",
        "series": {
            "title": "Breaking Bad"
        },
        "episodeFile": {
            "path": episode_file_str,
            "quality": "HDTV-720p"
        }
    });

    let resp = client
        .post(format!("http://{addr}/webhook/sonarr"))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json["job_id"].is_string());

    let conn = harness.conn();
    let jobs = sf_db::queries::jobs::list_jobs(&conn, Some("queued"), 0, 100).unwrap();
    assert_eq!(jobs.len(), 1);
    assert!(jobs[0].file_path.contains("Breaking Bad"));
    assert_eq!(jobs[0].source.as_deref(), Some("sonarr"));
}

// ---------------------------------------------------------------------------
// Webhook with no file path returns acknowledgement
// ---------------------------------------------------------------------------

#[tokio::test]
async fn webhook_without_file_path_acknowledges() {
    let (_harness, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();

    // Use a processable event type but without a file path -- should be
    // acknowledged (not ignored, which is for non-processable event types).
    let payload = serde_json::json!({
        "eventType": "Download",
        "movie": {
            "title": "Some Movie"
        }
    });

    let resp = client
        .post(format!("http://{addr}/webhook/radarr"))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["status"], "acknowledged");
}

#[tokio::test]
async fn webhook_non_processable_event_is_ignored() {
    let (_harness, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();

    // Non-processable event types (e.g. "Test") are acknowledged with
    // status "ignored".
    let payload = serde_json::json!({
        "eventType": "Test",
        "message": "Hello from Radarr"
    });

    let resp = client
        .post(format!("http://{addr}/webhook/radarr"))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["status"], "ignored");
}

// ---------------------------------------------------------------------------
// Signature verification (valid and invalid)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn webhook_with_valid_signature() {
    let mut config = sf_core::config::Config::default();
    config.webhook_security.signature_verification = true;
    config.webhook_security.signature_secret = Some("my-secret".into());

    let (_harness, addr) = TestHarness::with_server_config(config).await;
    let client = reqwest::Client::new();

    let payload = serde_json::json!({"eventType": "Test"});
    let body_bytes = serde_json::to_vec(&payload).unwrap();

    // Compute a real HMAC-SHA256 signature for the payload.
    let signature = compute_signature("my-secret", &body_bytes);

    let resp = client
        .post(format!("http://{addr}/webhook/radarr"))
        .header("x-sceneforged-signature", &signature)
        .header("Content-Type", "application/json")
        .body(body_bytes)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn webhook_with_missing_signature_returns_401() {
    let mut config = sf_core::config::Config::default();
    config.webhook_security.signature_verification = true;
    config.webhook_security.signature_secret = Some("my-secret".into());

    let (_harness, addr) = TestHarness::with_server_config(config).await;
    let client = reqwest::Client::new();

    let payload = serde_json::json!({"eventType": "Test"});

    // No signature header -- should be rejected.
    let resp = client
        .post(format!("http://{addr}/webhook/radarr"))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

// ---------------------------------------------------------------------------
// Invalid JSON body
// ---------------------------------------------------------------------------

#[tokio::test]
async fn webhook_with_invalid_json() {
    let (_harness, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://{addr}/webhook/radarr"))
        .header("Content-Type", "application/json")
        .body("not valid json")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}
