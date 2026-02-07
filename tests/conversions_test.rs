//! Integration tests for conversion job routes.

mod common;

use common::TestHarness;

#[tokio::test]
async fn submit_conversion() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, _, item_id_str, _) = h.create_item_with_media(lib_id, "Test Movie", "movie");

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/conversions/submit"))
        .json(&serde_json::json!({"item_id": item_id_str}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["status"], "queued");
    assert_eq!(json["item_id"], item_id_str);
    assert!(json["id"].is_string());
    assert!(json["source_media_file_id"].is_string());
}

#[tokio::test]
async fn submit_conversion_item_not_found() {
    let (_h, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/conversions/submit"))
        .json(&serde_json::json!({
            "item_id": "00000000-0000-0000-0000-000000000001"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn submit_duplicate_blocked() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, _, item_id_str, _) = h.create_item_with_media(lib_id, "Dup Movie", "movie");

    let client = reqwest::Client::new();

    // First submit succeeds.
    let resp = client
        .post(format!("http://{addr}/api/conversions/submit"))
        .json(&serde_json::json!({"item_id": item_id_str}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    // Second submit should be blocked (conflict).
    let resp = client
        .post(format!("http://{addr}/api/conversions/submit"))
        .json(&serde_json::json!({"item_id": item_id_str}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn list_conversions() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, _, item_id_str, _) = h.create_item_with_media(lib_id, "List Movie", "movie");

    let client = reqwest::Client::new();
    client
        .post(format!("http://{addr}/api/conversions/submit"))
        .json(&serde_json::json!({"item_id": item_id_str}))
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!("http://{addr}/api/conversions"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let jobs: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["item_name"], "List Movie");
}

#[tokio::test]
async fn list_conversions_filtered() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, _, item_id_str, _) = h.create_item_with_media(lib_id, "Filter Movie", "movie");

    let client = reqwest::Client::new();
    client
        .post(format!("http://{addr}/api/conversions/submit"))
        .json(&serde_json::json!({"item_id": item_id_str}))
        .send()
        .await
        .unwrap();

    // Filter by status=queued.
    let resp = client
        .get(format!("http://{addr}/api/conversions?status=queued"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let jobs: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(jobs.len(), 1);

    // Filter by status=completed (should be empty).
    let resp = client
        .get(format!(
            "http://{addr}/api/conversions?status=completed"
        ))
        .send()
        .await
        .unwrap();
    let jobs: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(jobs.is_empty());
}

#[tokio::test]
async fn get_conversion() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, _, item_id_str, _) = h.create_item_with_media(lib_id, "Get Conv", "movie");

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/conversions/submit"))
        .json(&serde_json::json!({"item_id": item_id_str}))
        .send()
        .await
        .unwrap();
    let created: serde_json::Value = resp.json().await.unwrap();
    let job_id = created["id"].as_str().unwrap();

    let resp = client
        .get(format!("http://{addr}/api/conversions/{job_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["id"], job_id);
    assert_eq!(json["status"], "queued");
}

#[tokio::test]
async fn get_conversion_not_found() {
    let (_h, addr) = TestHarness::with_server().await;
    let resp = reqwest::get(format!(
        "http://{addr}/api/conversions/00000000-0000-0000-0000-000000000001"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn delete_queued_conversion() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, _, item_id_str, _) = h.create_item_with_media(lib_id, "Del Conv", "movie");

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/conversions/submit"))
        .json(&serde_json::json!({"item_id": item_id_str}))
        .send()
        .await
        .unwrap();
    let created: serde_json::Value = resp.json().await.unwrap();
    let job_id = created["id"].as_str().unwrap();

    let resp = client
        .delete(format!("http://{addr}/api/conversions/{job_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify it's gone.
    let resp = client
        .get(format!("http://{addr}/api/conversions/{job_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn batch_convert() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, _, id1, _) = h.create_item_with_media(lib_id, "Batch 1", "movie");
    let (_, _, id2, _) = h.create_item_with_media(lib_id, "Batch 2", "movie");
    let (_, _, id3, _) = h.create_item_with_media(lib_id, "Batch 3", "movie");

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/conversions/batch"))
        .json(&serde_json::json!({"item_ids": [id1, id2, id3]}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["job_ids"].as_array().unwrap().len(), 3);
    assert!(json["errors"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn batch_convert_partial_errors() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, _, id1, _) = h.create_item_with_media(lib_id, "Partial 1", "movie");

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/conversions/batch"))
        .json(&serde_json::json!({
            "item_ids": [id1, "00000000-0000-0000-0000-999999999999"]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["job_ids"].as_array().unwrap().len(), 1);
    assert_eq!(json["errors"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn dv_batch_skips_non_dv() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, _, id1, _) = h.create_item_with_media(lib_id, "NoDV", "movie");

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{addr}/api/conversions/dv-batch"))
        .json(&serde_json::json!({"item_ids": [id1]}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    // No DV7 files exist, so no jobs created.
    assert!(json["job_ids"].as_array().unwrap().is_empty());
    assert_eq!(json["errors"].as_array().unwrap().len(), 1);
}
