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

    // Check credentials against config.
    let valid = match (&auth_config.username, &auth_config.password_hash) {
        (Some(expected_user), Some(expected_hash)) => {
            payload.username == *expected_user && payload.password == *expected_hash
        }
        _ => false,
    };

    if !valid {
        return Err(sf_core::Error::Unauthorized("Invalid credentials".into()).into());
    }

    // Create a session token in the database.
    let conn =
        sf_db::pool::get_conn(&ctx.db).map_err(|e| sf_core::Error::Internal(e.to_string()))?;

    // Find or create the user record.
    let user = match sf_db::queries::users::get_user_by_username(&conn, &payload.username)? {
        Some(u) => u,
        None => sf_db::queries::users::create_user(&conn, &payload.username, "config", "admin")?,
    };

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
        });
    }

    let authenticated = if let Some(token) = extract_token(&headers) {
        if let Some(ref api_key) = auth_config.api_key {
            if token == *api_key {
                return Json(AuthStatusResponse {
                    auth_enabled: true,
                    authenticated: true,
                });
            }
        }

        if let Ok(conn) = sf_db::pool::get_conn(&ctx.db) {
            sf_db::queries::auth::get_token(&conn, &token)
                .ok()
                .flatten()
                .is_some()
        } else {
            false
        }
    } else {
        false
    };

    Json(AuthStatusResponse {
        auth_enabled: true,
        authenticated,
    })
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
