use crate::arr::{RadarrWebhook, SonarrWebhook, WebhookEvent};
use crate::config::ArrType;
use crate::server::auth::verify_webhook_signature;
use crate::server::AppContext;
use crate::state::JobSource;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use std::path::PathBuf;

const SIGNATURE_HEADER: &str = "x-sceneforged-signature";

pub fn webhook_routes(ctx: &AppContext) -> Router<AppContext> {
    // Use raw body handler if signature verification is enabled
    if ctx.config.server.webhook_security.signature_verification {
        Router::new().route("/:arr_name", post(handle_webhook_with_signature))
    } else {
        Router::new().route("/:arr_name", post(handle_webhook))
    }
}

async fn handle_webhook_with_signature(
    State(ctx): State<AppContext>,
    Path(arr_name): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let security = &ctx.config.server.webhook_security;

    // Verify signature if secret is configured
    if let Some(ref secret) = security.signature_secret {
        let signature = headers
            .get(SIGNATURE_HEADER)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    format!("Missing {} header", SIGNATURE_HEADER),
                )
            })?;

        if !verify_webhook_signature(secret, &body, signature) {
            tracing::warn!("Webhook signature verification failed for {}", arr_name);
            return Err((StatusCode::UNAUTHORIZED, "Invalid signature".to_string()));
        }
    }

    // Parse the body as JSON
    let payload: serde_json::Value = serde_json::from_slice(&body).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid JSON payload: {}", e),
        )
    })?;

    process_webhook(ctx, arr_name, payload).await
}

async fn handle_webhook(
    State(ctx): State<AppContext>,
    Path(arr_name): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    process_webhook(ctx, arr_name, payload).await
}

async fn process_webhook(
    ctx: AppContext,
    arr_name: String,
    payload: serde_json::Value,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Find the arr config
    let arr_config = ctx
        .config
        .arrs
        .iter()
        .find(|a| a.name.to_lowercase() == arr_name.to_lowercase())
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Unknown arr: {}", arr_name)))?;

    if !arr_config.enabled {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Arr integration is disabled".to_string(),
        ));
    }

    // Parse based on arr type
    let event: WebhookEvent = match arr_config.arr_type {
        ArrType::Radarr => {
            let webhook: RadarrWebhook = serde_json::from_value(payload).map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("Invalid Radarr payload: {}", e),
                )
            })?;

            tracing::info!(
                "Received Radarr webhook: {} for {}",
                webhook.event_type,
                webhook
                    .movie
                    .as_ref()
                    .map(|m| m.title.as_str())
                    .unwrap_or("unknown")
            );

            webhook.to_event(&arr_name)
        }
        ArrType::Sonarr => {
            let webhook: SonarrWebhook = serde_json::from_value(payload).map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("Invalid Sonarr payload: {}", e),
                )
            })?;

            tracing::info!(
                "Received Sonarr webhook: {} for {}",
                webhook.event_type,
                webhook
                    .series
                    .as_ref()
                    .map(|s| s.title.as_str())
                    .unwrap_or("unknown")
            );

            webhook.to_event(&arr_name)
        }
    };

    // Only process certain event types
    let processable_events = [
        "Download",
        "Upgrade",
        "MovieFileDeleted",
        "EpisodeFileDeleted",
    ];
    if !processable_events
        .iter()
        .any(|e| event.event_type.contains(e))
    {
        tracing::debug!("Ignoring event type: {}", event.event_type);
        return Ok(Json(serde_json::json!({
            "status": "ignored",
            "reason": format!("Event type '{}' not processed", event.event_type)
        })));
    }

    // Queue the file for processing
    let file_path = event.file_path.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "No file path in webhook payload".to_string(),
        )
    })?;

    let path = PathBuf::from(&file_path);
    if !path.exists() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("File does not exist: {}", file_path),
        ));
    }

    let source = JobSource::Webhook {
        arr_name: event.arr_name.clone(),
        item_id: event.item_id,
    };

    match ctx.state.queue_job(path, source) {
        Ok(job) => {
            tracing::info!("Queued job {} for file: {}", job.id, file_path);
            Ok(Json(serde_json::json!({
                "status": "queued",
                "job_id": job.id,
                "file": file_path
            })))
        }
        Err(e) => {
            tracing::warn!("Failed to queue job: {}", e);
            Err((StatusCode::CONFLICT, e.to_string()))
        }
    }
}
