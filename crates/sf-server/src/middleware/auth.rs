//! Authentication middleware.
//!
//! Validates session cookies or API key bearer tokens. Skips authentication
//! for `/health` and `/api/auth/*` paths, and when auth is disabled in config.
//! Injects the authenticated [`UserId`] into request extensions so that
//! downstream handlers can access it.

use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use sf_core::UserId;
use sf_db::pool::DbPool;

use crate::context::AppContext;

/// Cookie name for browser sessions.
pub const SESSION_COOKIE: &str = "sceneforged_session";

/// Well-known user ID for unauthenticated requests (auth disabled).
/// Deterministic UUID v5 from the DNS namespace + "anonymous".
const ANONYMOUS_USER_ID: &str = "00000000-0000-0000-0000-000000000000";

/// Validate an auth token from raw HTTP header values.
///
/// Called by both [`auth_middleware`] (Axum) and the sendfile handler (raw TCP).
/// Returns `Some(UserId)` on success, `None` on failure.
pub fn validate_auth_headers(
    auth_config: &sf_core::config::AuthConfig,
    db: &DbPool,
    authorization: Option<&str>,
    cookie: Option<&str>,
) -> Option<UserId> {
    // If auth is not enabled, return anonymous user.
    if !auth_config.enabled {
        return Some(
            ANONYMOUS_USER_ID
                .parse()
                .expect("static anonymous UUID is valid"),
        );
    }

    // Check API key in Authorization: Bearer header.
    if let Some(auth_value) = authorization {
        if let Some(token) = auth_value.strip_prefix("Bearer ") {
            if let Some(ref api_key) = auth_config.api_key {
                if token == api_key {
                    return Some(
                        ANONYMOUS_USER_ID
                            .parse()
                            .expect("static anonymous UUID is valid"),
                    );
                }
            }

            // Also check against database auth tokens.
            if let Ok(conn) = sf_db::pool::get_conn(db) {
                if let Ok(Some(tok)) = sf_db::queries::auth::get_token(&conn, token) {
                    return Some(tok.user_id);
                }
            }
        }
    }

    // Check session cookie.
    if let Some(cookies_str) = cookie {
        for part in cookies_str.split(';') {
            let part = part.trim();
            if let Some(value) = part.strip_prefix(&format!("{SESSION_COOKIE}=")) {
                if let Ok(conn) = sf_db::pool::get_conn(db) {
                    if let Ok(Some(tok)) = sf_db::queries::auth::get_token(&conn, value) {
                        return Some(tok.user_id);
                    }
                }
            }
        }
    }

    None
}

/// Authentication middleware. Applied to protected routes only.
///
/// On success, inserts the resolved [`UserId`] into request extensions.
pub async fn auth_middleware(
    State(ctx): State<AppContext>,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, Response> {
    let authorization = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned());

    let cookie = request
        .headers()
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned());

    match validate_auth_headers(
        &ctx.config.auth,
        &ctx.db,
        authorization.as_deref(),
        cookie.as_deref(),
    ) {
        Some(user_id) => {
            request.extensions_mut().insert(user_id);
            Ok(next.run(request).await)
        }
        None => Err((StatusCode::UNAUTHORIZED, "Authentication required").into_response()),
    }
}
