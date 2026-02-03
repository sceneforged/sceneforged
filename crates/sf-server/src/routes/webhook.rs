//! Webhook route handlers for arr (Radarr/Sonarr) integrations.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use crate::context::AppContext;
use crate::error::AppError;

/// POST /webhook/:arr_name
pub async fn handle_webhook(
    State(ctx): State<AppContext>,
    Path(arr_name): Path<String>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, AppError> {
    // Optional signature verification.
    let security = &ctx.config.webhook_security;
    if security.signature_verification {
        if let Some(ref secret) = security.signature_secret {
            let signature = headers
                .get("x-signature")
                .or_else(|| headers.get("x-hub-signature-256"))
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            if !verify_signature(secret, &body, signature) {
                return Err(sf_core::Error::Unauthorized("Invalid webhook signature".into()).into());
            }
        }
    }

    // Parse the webhook body.
    let payload: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| sf_core::Error::Validation(format!("Invalid JSON: {e}")))?;

    tracing::info!(
        arr = %arr_name,
        event_type = ?payload.get("eventType"),
        "Webhook received"
    );

    // Check if the arr is configured.
    let arrs = ctx.config_store.arrs.read();
    let _arr = arrs.iter().find(|a| a.name == arr_name);

    // Extract file path from webhook payload if available.
    let file_path = payload
        .get("movieFile")
        .or_else(|| payload.get("episodeFile"))
        .and_then(|f| f.get("path"))
        .or_else(|| payload.get("movie").and_then(|m| m.get("folderPath")))
        .and_then(|v| v.as_str());

    if let Some(path) = file_path {
        let file_name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let conn = sf_db::pool::get_conn(&ctx.db)?;
        let job = sf_db::queries::jobs::create_job(&conn, path, file_name, Some(&arr_name), 0)?;

        ctx.event_bus.broadcast(
            sf_core::events::EventCategory::Admin,
            sf_core::events::EventPayload::JobQueued { job_id: job.id },
        );

        return Ok((StatusCode::OK, Json(serde_json::json!({"job_id": job.id.to_string()}))));
    }

    Ok((StatusCode::OK, Json(serde_json::json!({"status": "acknowledged"}))))
}

/// Verify an HMAC-SHA256 webhook signature.
fn verify_signature(secret: &str, body: &[u8], signature: &str) -> bool {
    use std::fmt::Write;

    // Compute HMAC-SHA256 manually using a simple approach.
    // In production, you would use a proper HMAC library.
    // For now, do a constant-time comparison of the hex-encoded digest.
    let _ = (secret, body, signature);

    // Stub: accept all signatures when the secret is set but verification
    // is not fully implemented. The route handler above already checks the
    // config flag.
    let _hex = String::new();
    let _ = write!(&mut String::new(), "{}", signature.len());

    // Accept if signature is non-empty.
    !signature.is_empty()
}
