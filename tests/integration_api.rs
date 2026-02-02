//! API integration tests
//!
//! Tests for HTTP API endpoints using axum's test utilities.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use parking_lot::RwLock;
use sceneforged::config::Config;
use sceneforged::server::{create_router, AppContext};
use sceneforged::state::{AppState, JobSource};
use std::sync::Arc;
use tempfile::tempdir;
use tower::ServiceExt;

/// Create a test context with default configuration
fn create_test_context() -> AppContext {
    let state = AppState::new(None);
    let config = Config::default();

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

/// Create a test context with custom configuration
#[allow(dead_code)]
fn create_test_context_with_config(config: Config) -> AppContext {
    let state = AppState::new(None);

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

/// Helper to get response body as string
async fn body_to_string(body: Body) -> String {
    let bytes = body.collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn test_health_endpoint() {
    let ctx = create_test_context();
    let app = create_router(ctx, None);

    let response = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_api_health_endpoint() {
    let ctx = create_test_context();
    let app = create_router(ctx, None);

    let response = app
        .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_to_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert_eq!(json["status"], "healthy");
    assert!(json["version"].is_string());
}

#[tokio::test]
async fn test_api_stats_endpoint() {
    let ctx = create_test_context();
    let app = create_router(ctx, None);

    let response = app
        .oneshot(Request::get("/api/stats").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_to_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert_eq!(json["total_processed"], 0);
    assert_eq!(json["successful"], 0);
    assert_eq!(json["failed"], 0);
}

#[tokio::test]
async fn test_api_jobs_empty() {
    let ctx = create_test_context();
    let app = create_router(ctx, None);

    let response = app
        .oneshot(Request::get("/api/jobs").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_to_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert!(json.is_array());
    assert_eq!(json.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_api_queue_empty() {
    let ctx = create_test_context();
    let app = create_router(ctx, None);

    let response = app
        .oneshot(Request::get("/api/queue").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_to_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert!(json.is_array());
    assert_eq!(json.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_api_history_empty() {
    let ctx = create_test_context();
    let app = create_router(ctx, None);

    let response = app
        .oneshot(Request::get("/api/history").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_to_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert!(json.is_array());
}

#[tokio::test]
async fn test_api_rules_empty() {
    let ctx = create_test_context();
    let app = create_router(ctx, None);

    let response = app
        .oneshot(Request::get("/api/rules").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_to_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert!(json.is_array());
    assert_eq!(json.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_api_arrs_empty() {
    let ctx = create_test_context();
    let app = create_router(ctx, None);

    let response = app
        .oneshot(Request::get("/api/arrs").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_to_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert!(json.is_array());
    assert_eq!(json.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_api_tools_endpoint() {
    let ctx = create_test_context();
    let app = create_router(ctx, None);

    let response = app
        .oneshot(Request::get("/api/tools").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_to_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();

    // Should return an array of tools
    assert!(json.is_array());

    // Check structure of tool entries
    for tool in json.as_array().unwrap() {
        assert!(tool["name"].is_string());
        assert!(tool["available"].is_boolean());
    }
}

#[tokio::test]
async fn test_api_get_nonexistent_job() {
    let ctx = create_test_context();
    let app = create_router(ctx, None);

    let response = app
        .oneshot(
            Request::get("/api/jobs/00000000-0000-0000-0000-000000000000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_api_delete_nonexistent_job() {
    let ctx = create_test_context();
    let app = create_router(ctx, None);

    let response = app
        .oneshot(
            Request::delete("/api/jobs/00000000-0000-0000-0000-000000000000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_jobs_lifecycle() {
    let temp = tempdir().unwrap();
    let test_file = temp.path().join("test_movie.mkv");
    std::fs::write(&test_file, b"fake video content").unwrap();

    let ctx = create_test_context();

    // Queue a job directly through state
    let job = ctx
        .state
        .queue_job(test_file.clone(), JobSource::Api)
        .unwrap();

    // Verify job appears in active jobs
    let jobs = ctx.state.get_active_jobs();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].id, job.id);
    assert_eq!(jobs[0].file_name, "test_movie.mkv");

    // Verify job is in queue
    let queue = ctx.state.get_queue();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0], job.id);

    // Get job by ID
    let retrieved = ctx.state.get_job(job.id).unwrap();
    assert_eq!(retrieved.id, job.id);

    // Start the job
    ctx.state.start_job(job.id, "Test Rule");
    let updated = ctx.state.get_job(job.id).unwrap();
    assert_eq!(updated.rule_name, Some("Test Rule".to_string()));

    // Update progress
    ctx.state.update_progress(job.id, 50.0, "Processing");
    let updated = ctx.state.get_job(job.id).unwrap();
    assert_eq!(updated.progress, 50.0);
    assert_eq!(updated.current_step, Some("Processing".to_string()));

    // Complete the job
    ctx.state.complete_job(job.id);

    // Job should no longer be in active jobs
    let jobs = ctx.state.get_active_jobs();
    assert_eq!(jobs.len(), 0);

    // But should be in history
    let history = ctx.state.get_history(10);
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].id, job.id);
}

#[tokio::test]
async fn test_job_failure() {
    let temp = tempdir().unwrap();
    let test_file = temp.path().join("test_movie.mkv");
    std::fs::write(&test_file, b"fake video content").unwrap();

    let ctx = create_test_context();

    let job = ctx
        .state
        .queue_job(test_file.clone(), JobSource::Api)
        .unwrap();

    // Start and fail the job
    ctx.state.start_job(job.id, "Test Rule");
    ctx.state.fail_job(job.id, "Test error message");

    // Job should be in history with failed status
    let history = ctx.state.get_history(10);
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].error, Some("Test error message".to_string()));

    // Stats should show failure
    let stats = ctx.state.get_stats();
    assert_eq!(stats.total_processed, 1);
    assert_eq!(stats.failed, 1);
}

#[tokio::test]
async fn test_duplicate_job_rejection() {
    let temp = tempdir().unwrap();
    let test_file = temp.path().join("test_movie.mkv");
    std::fs::write(&test_file, b"fake video content").unwrap();

    let ctx = create_test_context();

    // First queue should succeed
    let result1 = ctx.state.queue_job(test_file.clone(), JobSource::Api);
    assert!(result1.is_ok());

    // Second queue for same file should fail
    let result2 = ctx.state.queue_job(test_file.clone(), JobSource::Api);
    assert!(result2.is_err());
}
