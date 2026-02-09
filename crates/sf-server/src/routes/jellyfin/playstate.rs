//! Jellyfin-compatible playback state reporting endpoints.
//!
//! These map the Jellyfin sessions/playstate protocol to our internal
//! playback tracking.

use axum::extract::{Path, State};
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

/// POST /Sessions/Capabilities/Full — client registers its capabilities.
///
/// Infuse sends this immediately after auth. We accept and discard the body.
pub async fn capabilities_full(
    _headers: HeaderMap,
) -> StatusCode {
    StatusCode::NO_CONTENT
}

/// POST /Sessions/Capabilities — simplified capability registration.
pub async fn capabilities(
    _headers: HeaderMap,
) -> StatusCode {
    StatusCode::NO_CONTENT
}

/// POST /Sessions/Playing/Ping — keep session alive during playback.
pub async fn playing_ping() -> StatusCode {
    StatusCode::NO_CONTENT
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

/// POST /Users/{userId}/PlayedItems/{itemId} — mark item as played.
pub async fn mark_played(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((_user_id, item_id)): Path<(String, String)>,
) -> Result<Json<super::dto::UserDataDto>, AppError> {
    let user_id = resolve_user_from_headers(&ctx, &headers);
    let item_id: sf_core::ItemId = item_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid itemId".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    sf_db::queries::playback::upsert_playback(&conn, user_id, item_id, 0.0, true)?;

    let is_fav = sf_db::queries::favorites::get_favorite(&conn, user_id, item_id)?
        .is_some();

    Ok(Json(super::dto::UserDataDto {
        played: true,
        playback_position_ticks: 0,
        play_count: 1,
        is_favorite: is_fav,
        key: item_id.to_string(),
    }))
}

/// DELETE /Users/{userId}/PlayedItems/{itemId} — mark item as unplayed.
pub async fn mark_unplayed(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((_user_id, item_id)): Path<(String, String)>,
) -> Result<Json<super::dto::UserDataDto>, AppError> {
    let user_id = resolve_user_from_headers(&ctx, &headers);
    let item_id: sf_core::ItemId = item_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid itemId".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    sf_db::queries::playback::upsert_playback(&conn, user_id, item_id, 0.0, false)?;

    let is_fav = sf_db::queries::favorites::get_favorite(&conn, user_id, item_id)?
        .is_some();

    Ok(Json(super::dto::UserDataDto {
        played: false,
        playback_position_ticks: 0,
        play_count: 0,
        is_favorite: is_fav,
        key: item_id.to_string(),
    }))
}

/// POST /Users/{userId}/FavoriteItems/{itemId} — add to favorites.
pub async fn add_favorite(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((_user_id, item_id)): Path<(String, String)>,
) -> Result<Json<super::dto::UserDataDto>, AppError> {
    let user_id = resolve_user_from_headers(&ctx, &headers);
    let item_id: sf_core::ItemId = item_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid itemId".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    sf_db::queries::favorites::add_favorite(&conn, user_id, item_id)?;

    // Fetch current playback state for the response.
    let user_data_map =
        sf_db::queries::playback::batch_get_user_data(&conn, user_id, &[item_id])?;
    let ud = user_data_map.get(&item_id);

    let played = ud.map_or(false, |u| u.completed);
    let position_ticks = ud
        .map(|u| if u.position_secs > 0.0 { (u.position_secs * TICKS_PER_SECOND as f64) as i64 } else { 0 })
        .unwrap_or(0);

    Ok(Json(super::dto::UserDataDto {
        played,
        playback_position_ticks: position_ticks,
        play_count: if played { 1 } else { 0 },
        is_favorite: true,
        key: item_id.to_string(),
    }))
}

/// DELETE /Users/{userId}/FavoriteItems/{itemId} — remove from favorites.
pub async fn remove_favorite(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((_user_id, item_id)): Path<(String, String)>,
) -> Result<Json<super::dto::UserDataDto>, AppError> {
    let user_id = resolve_user_from_headers(&ctx, &headers);
    let item_id: sf_core::ItemId = item_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid itemId".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    sf_db::queries::favorites::remove_favorite(&conn, user_id, item_id)?;

    // Fetch current playback state for the response.
    let user_data_map =
        sf_db::queries::playback::batch_get_user_data(&conn, user_id, &[item_id])?;
    let ud = user_data_map.get(&item_id);

    let played = ud.map_or(false, |u| u.completed);
    let position_ticks = ud
        .map(|u| if u.position_secs > 0.0 { (u.position_secs * TICKS_PER_SECOND as f64) as i64 } else { 0 })
        .unwrap_or(0);

    Ok(Json(super::dto::UserDataDto {
        played,
        playback_position_ticks: position_ticks,
        play_count: if played { 1 } else { 0 },
        is_favorite: false,
        key: item_id.to_string(),
    }))
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
