//! Webhook integration tests
//!
//! Tests for Radarr and Sonarr webhook parsing and handling.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use parking_lot::RwLock;
use sceneforged::arr::{RadarrWebhook, SonarrWebhook};
use sceneforged::config::{ArrConfig, ArrType, Config, ServerConfig};
use sceneforged::server::{create_router, AppContext};
use sceneforged::state::AppState;
use std::sync::Arc;
use tempfile::tempdir;
use tower::ServiceExt;

/// Helper to get response body as string
#[allow(dead_code)]
async fn body_to_string(body: Body) -> String {
    let bytes = body.collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// Create a test context with Radarr configured
fn create_radarr_context() -> AppContext {
    let state = AppState::new(None);
    let config = Config {
        server: ServerConfig::default(),
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
    }
}

/// Create a test context with Sonarr configured
fn create_sonarr_context() -> AppContext {
    let state = AppState::new(None);
    let config = Config {
        server: ServerConfig::default(),
        watch: Default::default(),
        arrs: vec![ArrConfig {
            name: "sonarr".to_string(),
            arr_type: ArrType::Sonarr,
            url: "http://localhost:8989".to_string(),
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
    }
}

#[test]
fn test_radarr_webhook_parsing() {
    let payload = serde_json::json!({
        "eventType": "Download",
        "movie": {
            "id": 1,
            "title": "Test Movie",
            "filePath": "/movies/test.mkv",
            "folderPath": "/movies"
        },
        "movieFile": {
            "id": 1,
            "relativePath": "test.mkv",
            "path": "/movies/test.mkv"
        }
    });

    let webhook: RadarrWebhook = serde_json::from_value(payload).unwrap();

    assert_eq!(webhook.event_type, "Download");
    assert_eq!(webhook.movie.as_ref().unwrap().title, "Test Movie");
    assert_eq!(
        webhook.movie_file.as_ref().unwrap().path,
        Some("/movies/test.mkv".to_string())
    );
}

#[test]
fn test_radarr_webhook_to_event() {
    let payload = serde_json::json!({
        "eventType": "Download",
        "movie": {
            "id": 1,
            "title": "Test Movie"
        },
        "movieFile": {
            "id": 1,
            "path": "/movies/test.mkv"
        }
    });

    let webhook: RadarrWebhook = serde_json::from_value(payload).unwrap();
    let event = webhook.to_event("radarr");

    assert_eq!(event.arr_name, "radarr");
    assert_eq!(event.event_type, "Download");
    assert_eq!(event.title, "Test Movie");
    assert_eq!(event.file_path, Some("/movies/test.mkv".to_string()));
}

#[test]
fn test_sonarr_webhook_parsing() {
    let payload = serde_json::json!({
        "eventType": "Download",
        "series": {
            "id": 1,
            "title": "Test Series",
            "path": "/tv/Test Series"
        },
        "episodes": [{
            "id": 1,
            "episodeNumber": 5,
            "seasonNumber": 2,
            "title": "Test Episode"
        }],
        "episodeFile": {
            "id": 1,
            "relativePath": "Season 2/Test Series S02E05.mkv",
            "path": "/tv/Test Series/Season 2/Test Series S02E05.mkv"
        }
    });

    let webhook: SonarrWebhook = serde_json::from_value(payload).unwrap();

    assert_eq!(webhook.event_type, "Download");
    assert_eq!(webhook.series.as_ref().unwrap().title, "Test Series");
    assert_eq!(
        webhook.episode_file.as_ref().unwrap().path,
        Some("/tv/Test Series/Season 2/Test Series S02E05.mkv".to_string())
    );
}

#[test]
fn test_sonarr_webhook_to_event() {
    let payload = serde_json::json!({
        "eventType": "Download",
        "series": {
            "id": 1,
            "title": "Test Series"
        },
        "episodeFile": {
            "id": 1,
            "path": "/tv/test.mkv"
        }
    });

    let webhook: SonarrWebhook = serde_json::from_value(payload).unwrap();
    let event = webhook.to_event("sonarr");

    assert_eq!(event.arr_name, "sonarr");
    assert_eq!(event.event_type, "Download");
    assert_eq!(event.title, "Test Series");
    assert_eq!(event.file_path, Some("/tv/test.mkv".to_string()));
}

#[tokio::test]
async fn test_webhook_unknown_arr() {
    let ctx = create_radarr_context();
    let app = create_router(ctx, None);

    let payload = serde_json::json!({
        "eventType": "Download",
        "movie": {
            "id": 1,
            "title": "Test"
        }
    });

    let response = app
        .oneshot(
            Request::post("/webhook/unknown-arr")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_webhook_disabled_arr() {
    let state = AppState::new(None);
    let config = Config {
        server: ServerConfig::default(),
        watch: Default::default(),
        arrs: vec![ArrConfig {
            name: "radarr".to_string(),
            arr_type: ArrType::Radarr,
            url: "http://localhost:7878".to_string(),
            api_key: "test-key".to_string(),
            enabled: false, // Disabled
            auto_rescan: true,
            auto_rename: false,
        }],
        rules: vec![],
        jellyfins: vec![],
        tools: Default::default(),
    };

    let ctx = AppContext {
        state,
        rules: Arc::new(RwLock::new(config.rules.clone())),
        arrs: Arc::new(RwLock::new(config.arrs.clone())),
        jellyfins: Arc::new(RwLock::new(config.jellyfins.clone())),
        config: Arc::new(config),
        config_path: None,
        db_pool: None,
    };

    let app = create_router(ctx, None);

    let payload = serde_json::json!({
        "eventType": "Download",
        "movie": {
            "id": 1,
            "title": "Test"
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

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_webhook_invalid_payload() {
    let ctx = create_radarr_context();
    let app = create_router(ctx, None);

    let response = app
        .oneshot(
            Request::post("/webhook/radarr")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("not valid json"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_webhook_ignored_event_type() {
    let ctx = create_radarr_context();
    let app = create_router(ctx, None);

    // "Test" is not a processable event type
    let payload = serde_json::json!({
        "eventType": "Test",
        "movie": {
            "id": 1,
            "title": "Test Movie"
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

    assert_eq!(json["status"], "ignored");
}

#[tokio::test]
async fn test_webhook_download_queues_job() {
    let temp = tempdir().unwrap();
    let test_file = temp.path().join("test_movie.mkv");
    std::fs::write(&test_file, b"fake video content").unwrap();

    let ctx = create_radarr_context();
    let app = create_router(ctx.clone(), None);

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
    assert!(json["job_id"].is_string());

    // Verify job was actually queued
    let jobs = ctx.state.get_active_jobs();
    assert_eq!(jobs.len(), 1);
}

#[tokio::test]
async fn test_webhook_file_not_found() {
    let ctx = create_radarr_context();
    let app = create_router(ctx, None);

    let payload = serde_json::json!({
        "eventType": "Download",
        "movie": {
            "id": 1,
            "title": "Test Movie"
        },
        "movieFile": {
            "id": 1,
            "path": "/nonexistent/path/movie.mkv"
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

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_webhook_sonarr_download() {
    let temp = tempdir().unwrap();
    let test_file = temp.path().join("episode.mkv");
    std::fs::write(&test_file, b"fake video content").unwrap();

    let ctx = create_sonarr_context();
    let app = create_router(ctx.clone(), None);

    let payload = serde_json::json!({
        "eventType": "Download",
        "series": {
            "id": 1,
            "title": "Test Series"
        },
        "episodeFile": {
            "id": 1,
            "path": test_file.to_str().unwrap()
        }
    });

    let response = app
        .oneshot(
            Request::post("/webhook/sonarr")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify job was queued
    let jobs = ctx.state.get_active_jobs();
    assert_eq!(jobs.len(), 1);
}
