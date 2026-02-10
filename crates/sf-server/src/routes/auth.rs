//! Authentication route handlers: login, logout, status.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::context::AppContext;
use crate::error::AppError;
use crate::middleware::auth::SESSION_COOKIE;

/// Login request payload.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Login/status response.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct AuthResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

/// Auth status response.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct AuthStatusResponse {
    pub auth_enabled: bool,
    pub authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

/// Password change request.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// POST /api/auth/login
#[utoipa::path(
    post,
    path = "/api/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 401, description = "Invalid credentials")
    )
)]
pub async fn login(
    State(ctx): State<AppContext>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let auth_config = &ctx.config.auth;

    if !auth_config.enabled {
        return Ok((
            StatusCode::OK,
            Json(AuthResponse {
                success: true,
                message: "Auth disabled".into(),
                token: None,
            }),
        ));
    }

    let conn =
        sf_db::pool::get_conn(&ctx.db).map_err(|e| sf_core::Error::Internal(e.to_string()))?;

    // Look up user in the database.
    let user = match sf_db::queries::users::get_user_by_username(&conn, &payload.username)? {
        Some(u) => u,
        None => {
            // Fall back to config-based auth (legacy single-user mode).
            match (&auth_config.username, &auth_config.password_hash) {
                (Some(expected_user), Some(expected_hash))
                    if payload.username == *expected_user && payload.password == *expected_hash =>
                {
                    // Auto-migrate: create user with bcrypt hash.
                    let hash = bcrypt::hash(&payload.password, bcrypt::DEFAULT_COST)
                        .map_err(|e| sf_core::Error::Internal(format!("bcrypt error: {e}")))?;
                    sf_db::queries::users::create_user(&conn, &payload.username, &hash, "admin")?
                }
                _ => {
                    return Err(
                        sf_core::Error::Unauthorized("Invalid credentials".into()).into(),
                    );
                }
            }
        }
    };

    // Verify password against stored bcrypt hash.
    // If the hash is "config" or "!disabled" (legacy), skip bcrypt check but
    // verify against config for backwards compat.
    let password_valid = if user.password_hash.starts_with("$2") {
        bcrypt::verify(&payload.password, &user.password_hash).unwrap_or(false)
    } else {
        // Legacy: plaintext or config-based password.
        match &auth_config.password_hash {
            Some(expected) => payload.password == *expected,
            None => false,
        }
    };

    if !password_valid {
        return Err(sf_core::Error::Unauthorized("Invalid credentials".into()).into());
    }

    // Auto-upgrade: if password hash isn't bcrypt, upgrade it now.
    if !user.password_hash.starts_with("$2") {
        if let Ok(hash) = bcrypt::hash(&payload.password, bcrypt::DEFAULT_COST) {
            let _ = sf_db::queries::users::update_password(&conn, user.id, &hash);
        }
    }

    let token = uuid::Uuid::new_v4().to_string();
    let expires = Utc::now()
        + Duration::hours(ctx.config.auth.session_timeout_hours as i64);
    let expires_str = expires.to_rfc3339();

    sf_db::queries::auth::create_token(&conn, user.id, &token, &expires_str)?;

    Ok((
        StatusCode::OK,
        Json(AuthResponse {
            success: true,
            message: "Login successful".into(),
            token: Some(token),
        }),
    ))
}

/// POST /api/auth/logout
#[utoipa::path(
    post,
    path = "/api/auth/logout",
    responses(
        (status = 200, description = "Logged out")
    )
)]
pub async fn logout(
    State(ctx): State<AppContext>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    // Try to find the token from the Authorization header or cookie.
    let token = extract_token(&headers);

    if let Some(token) = token {
        if let Ok(conn) = sf_db::pool::get_conn(&ctx.db) {
            let _ = sf_db::queries::auth::delete_token(&conn, &token);
        }
    }

    Ok(StatusCode::OK)
}

/// GET /api/auth/status
#[utoipa::path(
    get,
    path = "/api/auth/status",
    responses(
        (status = 200, description = "Auth status", body = AuthStatusResponse)
    )
)]
pub async fn auth_status(
    State(ctx): State<AppContext>,
    headers: axum::http::HeaderMap,
) -> Json<AuthStatusResponse> {
    let auth_config = &ctx.config.auth;

    if !auth_config.enabled {
        return Json(AuthStatusResponse {
            auth_enabled: false,
            authenticated: true,
            user_id: None,
            username: None,
            role: Some("admin".into()),
        });
    }

    if let Some(token) = extract_token(&headers) {
        if let Some(ref api_key) = auth_config.api_key {
            if token == *api_key {
                return Json(AuthStatusResponse {
                    auth_enabled: true,
                    authenticated: true,
                    user_id: None,
                    username: None,
                    role: Some("admin".into()),
                });
            }
        }

        if let Ok(conn) = sf_db::pool::get_conn(&ctx.db) {
            if let Ok(Some(tok)) = sf_db::queries::auth::get_token(&conn, &token) {
                let user = sf_db::queries::users::get_user_by_id(&conn, tok.user_id)
                    .ok()
                    .flatten();
                return Json(AuthStatusResponse {
                    auth_enabled: true,
                    authenticated: true,
                    user_id: Some(tok.user_id.to_string()),
                    username: user.as_ref().map(|u| u.username.clone()),
                    role: user.map(|u| u.role),
                });
            }
        }
    }

    Json(AuthStatusResponse {
        auth_enabled: true,
        authenticated: false,
        user_id: None,
        username: None,
        role: None,
    })
}

/// PUT /api/auth/password
#[utoipa::path(
    put,
    path = "/api/auth/password",
    request_body = ChangePasswordRequest,
    responses(
        (status = 200, description = "Password changed"),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Current password incorrect")
    )
)]
pub async fn change_password(
    State(ctx): State<AppContext>,
    axum::extract::Extension(user_id): axum::extract::Extension<sf_core::UserId>,
    Json(payload): Json<ChangePasswordRequest>,
) -> Result<impl IntoResponse, AppError> {
    if payload.new_password.len() < 8 {
        return Err(sf_core::Error::Validation(
            "New password must be at least 8 characters".into(),
        )
        .into());
    }

    let conn =
        sf_db::pool::get_conn(&ctx.db).map_err(|e| sf_core::Error::Internal(e.to_string()))?;

    let user = sf_db::queries::users::get_user_by_id(&conn, user_id)?
        .ok_or_else(|| sf_core::Error::Unauthorized("User not found".into()))?;

    // Verify current password.
    let valid = if user.password_hash.starts_with("$2") {
        bcrypt::verify(&payload.current_password, &user.password_hash).unwrap_or(false)
    } else {
        false
    };

    if !valid {
        return Err(sf_core::Error::Unauthorized("Current password is incorrect".into()).into());
    }

    let new_hash = bcrypt::hash(&payload.new_password, bcrypt::DEFAULT_COST)
        .map_err(|e| sf_core::Error::Internal(format!("bcrypt error: {e}")))?;

    sf_db::queries::users::update_password(&conn, user.id, &new_hash)?;

    Ok(Json(AuthResponse {
        success: true,
        message: "Password changed".into(),
        token: None,
    }))
}

/// Extract a bearer token or session cookie from request headers.
fn extract_token(headers: &axum::http::HeaderMap) -> Option<String> {
    // Check Authorization header first.
    if let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(val) = auth.to_str() {
            if let Some(token) = val.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }

    // Check cookie.
    if let Some(cookie) = headers.get(axum::http::header::COOKIE) {
        if let Ok(cookies_str) = cookie.to_str() {
            for part in cookies_str.split(';') {
                let part = part.trim();
                if let Some(value) = part.strip_prefix(&format!("{SESSION_COOKIE}=")) {
                    return Some(value.to_string());
                }
            }
        }
    }

    None
}
