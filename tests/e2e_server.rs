//! Server end-to-end tests
//!
//! Tests for the full server lifecycle including webhooks and SSE.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use parking_lot::RwLock;
use sceneforged::config::{ArrConfig, ArrType, Config, ServerConfig};
use sceneforged::server::{create_router, AppContext};
use sceneforged::state::AppState;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::time::{timeout, Duration};
use tower::ServiceExt;

/// Helper to get response body as string
#[allow(dead_code)]
async fn body_to_string(body: Body) -> String {
    let bytes = body.collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// Create a full test context with Radarr and rules
fn create_full_context() -> AppContext {
    let state = AppState::new(None);
    let config = Config {
        server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 0,
            static_dir: None,
            auth: Default::default(),
            webhook_security: Default::default(),
        },
        watch: Default::default(),
        arrs: vec![ArrConfig {
            name: "radarr".to_string(),
            arr_type: ArrType::Radarr,
            url: "http://localhost:7878".to_string(),
            api_key: "test-key".to_string(),
            enabled: true,
            auto_rescan: true,
            auto_rename: false,
        }],
        rules: vec![],
        jellyfins: vec![],
        tools: Default::default(),
    };

    AppContext {
        state,
        rules: Arc::new(RwLock::new(config.rules.clone())),
        arrs: Arc::new(RwLock::new(config.arrs.clone())),
        jellyfins: Arc::new(RwLock::new(config.jellyfins.clone())),
        config: Arc::new(config),
        config_path: None,
        db_pool: None,
        session_manager: None,
        conversion_manager: None,
    }
}

#[tokio::test]
async fn test_full_webhook_to_job_flow() {
    let temp = tempdir().unwrap();
    let test_file = temp.path().join("movie.mkv");
    std::fs::write(&test_file, b"fake video content").unwrap();

    let ctx = create_full_context();

    // Subscribe to events before sending webhook
    let mut event_rx = ctx.state.subscribe();

    let app = create_router(ctx.clone(), None);

    // Send a download webhook
    let payload = serde_json::json!({
        "eventType": "Download",
        "movie": {
            "id": 1,
            "title": "Test Movie"
        },
        "movieFile": {
            "id": 1,
            "path": test_file.to_str().unwrap()
        }
    });

    let response = app
        .oneshot(
            Request::post("/webhook/radarr")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_to_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert_eq!(json["status"], "queued");
    let job_id = json["job_id"].as_str().unwrap();

    // Verify job is in queue
    let queue = ctx.state.get_queue();
    assert_eq!(queue.len(), 1);

    // Verify SSE event was emitted
    let event = timeout(Duration::from_millis(100), event_rx.recv())
        .await
        .expect("Should receive event")
        .expect("Event should not be error");

    match event {
        sceneforged::state::AppEvent::JobQueued { job, .. } => {
            assert_eq!(job.id.to_string(), job_id);
            assert_eq!(job.file_name, "movie.mkv");
        }
        _ => panic!("Expected JobQueued event"),
    }
}

#[tokio::test]
async fn test_job_progress_events() {
    let temp = tempdir().unwrap();
    let test_file = temp.path().join("movie.mkv");
    std::fs::write(&test_file, b"fake video content").unwrap();

    let ctx = create_full_context();

    // Queue a job
    let job = ctx
        .state
        .queue_job(test_file, sceneforged::state::JobSource::Api)
        .unwrap();

    // Subscribe after queuing to avoid the queue event
    let mut event_rx = ctx.state.subscribe();

    // Start the job
    ctx.state.start_job(job.id, "Test Rule");

    // Check for started event
    let event = timeout(Duration::from_millis(100), event_rx.recv())
        .await
        .expect("Should receive event")
        .expect("Event should not be error");

    match event {
        sceneforged::state::AppEvent::JobStarted { id, rule_name, .. } => {
            assert_eq!(id, job.id);
            assert_eq!(rule_name, "Test Rule");
        }
        _ => panic!("Expected JobStarted event, got {:?}", event),
    }

    // Update progress
    ctx.state.update_progress(job.id, 50.0, "Processing video");

    let event = timeout(Duration::from_millis(100), event_rx.recv())
        .await
        .expect("Should receive event")
        .expect("Event should not be error");

    match event {
        sceneforged::state::AppEvent::JobProgress { id, progress, step, .. } => {
            assert_eq!(id, job.id);
            assert_eq!(progress, 50.0);
            assert_eq!(step, "Processing video");
        }
        _ => panic!("Expected JobProgress event"),
    }

    // Complete the job
    ctx.state.complete_job(job.id);

    let event = timeout(Duration::from_millis(100), event_rx.recv())
        .await
        .expect("Should receive event")
        .expect("Event should not be error");

    match event {
        sceneforged::state::AppEvent::JobCompleted { job: completed_job, .. } => {
            assert_eq!(completed_job.id, job.id);
        }
        _ => panic!("Expected JobCompleted event"),
    }
}

#[tokio::test]
async fn test_job_failure_events() {
    let temp = tempdir().unwrap();
    let test_file = temp.path().join("movie.mkv");
    std::fs::write(&test_file, b"fake video content").unwrap();

    let ctx = create_full_context();

    let job = ctx
        .state
        .queue_job(test_file, sceneforged::state::JobSource::Api)
        .unwrap();

    let mut event_rx = ctx.state.subscribe();

    ctx.state.start_job(job.id, "Test Rule");
    // Consume the Started event
    let _ = event_rx.recv().await;

    // Fail the job
    ctx.state.fail_job(job.id, "ffmpeg error: invalid input");

    let event = timeout(Duration::from_millis(100), event_rx.recv())
        .await
        .expect("Should receive event")
        .expect("Event should not be error");

    match event {
        sceneforged::state::AppEvent::JobFailed { id, error, .. } => {
            assert_eq!(id, job.id);
            assert_eq!(error, "ffmpeg error: invalid input");
        }
        _ => panic!("Expected JobFailed event"),
    }

    // Verify stats reflect failure
    let stats = ctx.state.get_stats();
    assert_eq!(stats.failed, 1);
    assert_eq!(stats.total_processed, 1);
}

#[tokio::test]
async fn test_multiple_concurrent_jobs() {
    let temp = tempdir().unwrap();

    let ctx = create_full_context();

    // Create and queue multiple files
    let mut job_ids = Vec::new();
    for i in 0..5 {
        let test_file = temp.path().join(format!("movie_{}.mkv", i));
        std::fs::write(&test_file, b"fake video content").unwrap();

        let job = ctx
            .state
            .queue_job(test_file, sceneforged::state::JobSource::Api)
            .unwrap();
        job_ids.push(job.id);
    }

    // All jobs should be in queue
    let queue = ctx.state.get_queue();
    assert_eq!(queue.len(), 5);

    // Dequeue and process jobs
    for (i, expected_id) in job_ids.iter().enumerate() {
        let job = ctx.state.dequeue_job().unwrap();
        assert_eq!(job.id, *expected_id);

        ctx.state.start_job(job.id, &format!("Rule {}", i));
        ctx.state.complete_job(job.id);
    }

    // Queue should be empty
    let queue = ctx.state.get_queue();
    assert_eq!(queue.len(), 0);

    // All jobs should be in history
    let history = ctx.state.get_history(10);
    assert_eq!(history.len(), 5);

    // Stats should show 5 successful
    let stats = ctx.state.get_stats();
    assert_eq!(stats.successful, 5);
    assert_eq!(stats.total_processed, 5);
}

#[tokio::test]
async fn test_api_endpoint_sequence() {
    let temp = tempdir().unwrap();
    let test_file = temp.path().join("movie.mkv");
    std::fs::write(&test_file, b"fake video content").unwrap();

    let ctx = create_full_context();

    // Step 1: Health check
    let app = create_router(ctx.clone(), None);
    let response = app
        .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Step 2: Check empty queue
    let app = create_router(ctx.clone(), None);
    let response = app
        .oneshot(Request::get("/api/queue").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let body = body_to_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 0);

    // Step 3: Send webhook to queue a job
    let payload = serde_json::json!({
        "eventType": "Download",
        "movie": { "id": 1, "title": "Test Movie" },
        "movieFile": { "id": 1, "path": test_file.to_str().unwrap() }
    });

    let app = create_router(ctx.clone(), None);
    let response = app
        .oneshot(
            Request::post("/webhook/radarr")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Step 4: Verify job in queue
    let app = create_router(ctx.clone(), None);
    let response = app
        .oneshot(Request::get("/api/queue").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let body = body_to_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 1);

    // Step 5: Get job details
    let job_id = json[0]["id"].as_str().unwrap();
    let app = create_router(ctx.clone(), None);
    let response = app
        .oneshot(
            Request::get(format!("/api/jobs/{}", job_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Step 6: Complete the job (simulated)
    let uuid = uuid::Uuid::parse_str(job_id).unwrap();
    ctx.state.start_job(uuid, "Test Rule");
    ctx.state.complete_job(uuid);

    // Step 7: Verify job in history
    let app = create_router(ctx.clone(), None);
    let response = app
        .oneshot(Request::get("/api/history").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let body = body_to_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 1);

    // Step 8: Check updated stats
    let app = create_router(ctx.clone(), None);
    let response = app
        .oneshot(Request::get("/api/stats").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let body = body_to_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["successful"], 1);
}

#[tokio::test]
async fn test_retry_failed_job() {
    let temp = tempdir().unwrap();
    let test_file = temp.path().join("movie.mkv");
    std::fs::write(&test_file, b"fake video content").unwrap();

    let ctx = create_full_context();

    // Queue, start, and fail a job
    let job = ctx
        .state
        .queue_job(test_file.clone(), sceneforged::state::JobSource::Api)
        .unwrap();

    // Dequeue the job (simulating the processor picking it up)
    let _ = ctx.state.dequeue_job();

    ctx.state.start_job(job.id, "Test Rule");
    ctx.state.fail_job(job.id, "Test error");

    // Job should be in history
    let history = ctx.state.get_history(10);
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].id, job.id);

    // Retry the job via API
    let app = create_router(ctx.clone(), None);
    let response = app
        .oneshot(
            Request::post(format!("/api/jobs/{}/retry", job.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Should have a new job in queue (but with same file)
    let queue = ctx.state.get_queue();
    assert_eq!(queue.len(), 1);
    // The new job should have a different ID
    assert_ne!(queue[0], job.id);
}
