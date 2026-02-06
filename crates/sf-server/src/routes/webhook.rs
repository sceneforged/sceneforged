//! Webhook route handlers for arr (Radarr/Sonarr) integrations.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::context::AppContext;
use crate::error::AppError;

type HmacSha256 = Hmac<Sha256>;

/// Header names for webhook signature verification.
const SIGNATURE_HEADER: &str = "x-sceneforged-signature";
const HUB_SIGNATURE_HEADER: &str = "x-hub-signature-256";

/// Event types we actually process (all others are acknowledged but ignored).
const PROCESSABLE_EVENTS: &[&str] = &[
    "Download",
    "Upgrade",
    "MovieFileDeleted",
    "EpisodeFileDeleted",
];

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
                .get(SIGNATURE_HEADER)
                .or_else(|| headers.get(HUB_SIGNATURE_HEADER))
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            if !verify_signature(secret, &body, signature) {
                tracing::warn!(
                    arr = %arr_name,
                    "Webhook signature verification failed"
                );
                return Err(sf_core::Error::Unauthorized("Invalid webhook signature".into()).into());
            }
        }
    }

    // Parse the webhook body.
    let payload: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| sf_core::Error::Validation(format!("Invalid JSON: {e}")))?;

    let event_type = payload
        .get("eventType")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    tracing::info!(
        arr = %arr_name,
        event_type = %event_type,
        "Webhook received"
    );

    // Only process certain event types; acknowledge everything else.
    if !PROCESSABLE_EVENTS.iter().any(|e| event_type.contains(e)) {
        tracing::debug!(
            arr = %arr_name,
            event_type = %event_type,
            "Ignoring non-processable event type"
        );
        return Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "ignored",
                "reason": format!("Event type '{}' not processed", event_type)
            })),
        ));
    }

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
        // Validate file exists on disk before creating a job.
        let fs_path = std::path::Path::new(path);
        if !fs_path.exists() {
            tracing::warn!(
                arr = %arr_name,
                path = %path,
                "Webhook referenced file does not exist on disk"
            );
            return Err(sf_core::Error::Validation(format!(
                "File does not exist: {path}"
            ))
            .into());
        }

        let file_name = fs_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let conn = sf_db::pool::get_conn(&ctx.db)?;
        let job = sf_db::queries::jobs::create_job(&conn, path, file_name, Some(&arr_name), 0)?;

        ctx.event_bus.broadcast(
            sf_core::events::EventCategory::Admin,
            sf_core::events::EventPayload::JobQueued { job_id: job.id },
        );

        return Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "queued",
                "job_id": job.id.to_string(),
                "file": path
            })),
        ));
    }

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({"status": "acknowledged"})),
    ))
}

/// Verify an HMAC-SHA256 webhook signature.
///
/// Accepts signatures in the form `sha256=<hex>` or raw hex.
/// Uses constant-time comparison via the `hmac` crate.
fn verify_signature(secret: &str, body: &[u8], signature: &str) -> bool {
    if signature.is_empty() {
        return false;
    }

    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };

    mac.update(body);

    // Signature format: sha256=<hex> or raw hex.
    let hex_sig = signature.strip_prefix("sha256=").unwrap_or(signature);

    let expected_bytes = match hex::decode(hex_sig) {
        Ok(b) => b,
        Err(_) => return false,
    };

    mac.verify_slice(&expected_bytes).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_valid_signature() {
        let secret = "mysecret";
        let body = b"hello world";

        // Compute the expected signature.
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let result = mac.finalize();
        let hex_sig = hex::encode(result.into_bytes());

        assert!(verify_signature(secret, body, &hex_sig));
        assert!(verify_signature(
            secret,
            body,
            &format!("sha256={hex_sig}")
        ));
    }

    #[test]
    fn verify_invalid_signature() {
        assert!(!verify_signature("secret", b"body", "invalid"));
        assert!(!verify_signature("secret", b"body", ""));
        assert!(!verify_signature(
            "secret",
            b"body",
            "sha256=0000000000000000000000000000000000000000000000000000000000000000"
        ));
    }
}
