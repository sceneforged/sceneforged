//! Jellyfin-compatible playback state reporting endpoints.
//!
//! These map the Jellyfin sessions/playstate protocol to our internal
//! playback tracking.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;

use crate::context::AppContext;
use crate::error::AppError;
use crate::middleware::auth::validate_auth_headers;

use super::dto::TICKS_PER_SECOND;

/// Jellyfin play report — sent on start, progress, and stop.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlaystateReport {
    /// The item being played.
    pub item_id: Option<String>,
    /// Media source in use.
    pub media_source_id: Option<String>,
    /// Current position in ticks (100ns units).
    pub position_ticks: Option<i64>,
    /// Whether the item can seek.
    pub can_seek: Option<bool>,
    /// Whether the media is paused.
    pub is_paused: Option<bool>,
    /// Whether the media is muted.
    pub is_muted: Option<bool>,
    /// Volume level (0-100).
    pub volume_level: Option<i32>,
    /// Play session ID (from PlaybackInfo).
    pub play_session_id: Option<String>,
}

/// Well-known anonymous user ID (matches middleware/auth.rs).
fn anonymous_user_id() -> sf_core::UserId {
    "00000000-0000-0000-0000-000000000000"
        .parse()
        .expect("static anonymous UUID is valid")
}

/// Resolve a user ID from Jellyfin request headers.
/// Falls back to anonymous user if no valid token is found.
fn resolve_user_from_headers(ctx: &AppContext, headers: &HeaderMap) -> sf_core::UserId {
    let authorization = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    let cookie = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok());

    let x_emby_token = headers
        .get("X-Emby-Token")
        .and_then(|v| v.to_str().ok());

    validate_auth_headers(
        &ctx.config.auth,
        &ctx.db,
        authorization,
        cookie,
        x_emby_token,
    )
    .unwrap_or_else(anonymous_user_id)
}

/// POST /Sessions/Playing — client started playback.
pub async fn playing(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(report): Json<PlaystateReport>,
) -> Result<StatusCode, AppError> {
    let user_id = resolve_user_from_headers(&ctx, &headers);

    if let Some(ref id_str) = report.item_id {
        if let Ok(item_id) = id_str.parse::<sf_core::ItemId>() {
            let position_secs = report
                .position_ticks
                .map(|t| t as f64 / TICKS_PER_SECOND as f64)
                .unwrap_or(0.0);

            let conn = sf_db::pool::get_conn(&ctx.db)?;
            let _ = sf_db::queries::playback::upsert_playback(
                &conn,
                user_id,
                item_id,
                position_secs,
                false,
            );
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

/// POST /Sessions/Playing/Progress — client reporting progress.
pub async fn progress(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(report): Json<PlaystateReport>,
) -> Result<StatusCode, AppError> {
    let user_id = resolve_user_from_headers(&ctx, &headers);

    if let Some(ref id_str) = report.item_id {
        if let Ok(item_id) = id_str.parse::<sf_core::ItemId>() {
            let position_secs = report
                .position_ticks
                .map(|t| t as f64 / TICKS_PER_SECOND as f64)
                .unwrap_or(0.0);

            let conn = sf_db::pool::get_conn(&ctx.db)?;
            let _ = sf_db::queries::playback::upsert_playback(
                &conn,
                user_id,
                item_id,
                position_secs,
                false,
            );
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

/// POST /Sessions/Playing/Stopped — client stopped playback.
pub async fn stopped(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(report): Json<PlaystateReport>,
) -> Result<StatusCode, AppError> {
    let user_id = resolve_user_from_headers(&ctx, &headers);

    if let Some(ref id_str) = report.item_id {
        if let Ok(item_id) = id_str.parse::<sf_core::ItemId>() {
            let position_secs = report
                .position_ticks
                .map(|t| t as f64 / TICKS_PER_SECOND as f64)
                .unwrap_or(0.0);

            let conn = sf_db::pool::get_conn(&ctx.db)?;
            let _ = sf_db::queries::playback::upsert_playback(
                &conn,
                user_id,
                item_id,
                position_secs,
                false, // Jellyfin clients don't signal completion via stopped
            );
        }
    }
    Ok(StatusCode::NO_CONTENT)
}
