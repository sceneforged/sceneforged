//! Jellyfin user endpoints (AuthenticateByName, user info).

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::context::AppContext;
use crate::error::AppError;
use crate::middleware::auth::parse_mediabrowser_header;

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinUser {
    pub id: String,
    pub name: String,
    pub server_id: String,
    pub has_password: bool,
    pub has_configured_password: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthResult {
    pub user: JellyfinUser,
    pub access_token: String,
    pub server_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthByNameRequest {
    pub username: String,
    pub pw: String,
}

/// GET /Users/Public — list public users (for login screen).
pub async fn public_users(
    State(ctx): State<AppContext>,
) -> Result<Json<Vec<JellyfinUser>>, AppError> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let users = sf_db::queries::users::list_users(&conn)?;

    Ok(Json(
        users
            .into_iter()
            .filter(|u| u.username != "anonymous")
            .map(|u| JellyfinUser {
                id: u.id.to_string(),
                name: u.username,
                server_id: "sceneforged-server".into(),
                has_password: true,
                has_configured_password: true,
            })
            .collect(),
    ))
}

/// POST /Users/AuthenticateByName — Jellyfin login.
pub async fn authenticate_by_name(
    State(ctx): State<AppContext>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<AuthByNameRequest>,
) -> Result<Json<AuthResult>, AppError> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;

    let user = sf_db::queries::users::get_user_by_username(&conn, &payload.username)?
        .ok_or_else(|| sf_core::Error::Unauthorized("Invalid credentials".into()))?;

    // Verify password.
    let valid = if user.password_hash.starts_with("$2") {
        bcrypt::verify(&payload.pw, &user.password_hash).unwrap_or(false)
    } else {
        // Legacy config-based password.
        ctx.config
            .auth
            .password_hash
            .as_ref()
            .map_or(false, |h| payload.pw == *h)
    };

    if !valid {
        return Err(sf_core::Error::Unauthorized("Invalid credentials".into()).into());
    }

    // Create token, storing device info from MediaBrowser header.
    let _device_info = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(parse_mediabrowser_header);

    let token = uuid::Uuid::new_v4().to_string();
    let expires = chrono::Utc::now() + chrono::Duration::days(30);
    sf_db::queries::auth::create_token(&conn, user.id, &token, &expires.to_rfc3339())?;

    Ok(Json(AuthResult {
        user: JellyfinUser {
            id: user.id.to_string(),
            name: user.username,
            server_id: "sceneforged-server".into(),
            has_password: true,
            has_configured_password: true,
        },
        access_token: token,
        server_id: "sceneforged-server".into(),
    }))
}

/// GET /Users/{user_id} — get user info.
pub async fn get_user(
    State(ctx): State<AppContext>,
    Path(user_id): Path<String>,
) -> Result<Json<JellyfinUser>, AppError> {
    let id: sf_core::UserId = user_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid user_id".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let user = sf_db::queries::users::get_user_by_id(&conn, id)?
        .ok_or_else(|| sf_core::Error::not_found("user", id))?;

    Ok(Json(JellyfinUser {
        id: user.id.to_string(),
        name: user.username,
        server_id: "sceneforged-server".into(),
        has_password: true,
        has_configured_password: true,
    }))
}
