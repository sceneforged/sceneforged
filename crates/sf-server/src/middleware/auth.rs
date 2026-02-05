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

use crate::context::AppContext;

/// Cookie name for browser sessions.
pub const SESSION_COOKIE: &str = "sceneforged_session";

/// Well-known user ID for unauthenticated requests (auth disabled).
/// Deterministic UUID v5 from the DNS namespace + "anonymous".
const ANONYMOUS_USER_ID: &str = "00000000-0000-0000-0000-000000000000";

/// Authentication middleware. Applied to protected routes only.
///
/// On success, inserts the resolved [`UserId`] into request extensions.
pub async fn auth_middleware(
    State(ctx): State<AppContext>,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, Response> {
    let auth_config = &ctx.config.auth;

    // If auth is not enabled, inject anonymous user and pass through.
    if !auth_config.enabled {
        let anon_id: UserId = ANONYMOUS_USER_ID
            .parse()
            .expect("static anonymous UUID is valid");
        request.extensions_mut().insert(anon_id);
        return Ok(next.run(request).await);
    }

    // Check API key in Authorization: Bearer header.
    if let Some(auth_header) = request.headers().get(axum::http::header::AUTHORIZATION) {
        if let Ok(val) = auth_header.to_str() {
            if let Some(token) = val.strip_prefix("Bearer ") {
                if let Some(ref api_key) = auth_config.api_key {
                    if token == api_key {
                        // API key auth — resolve to first user or anonymous.
                        let user_id = resolve_api_key_user(&ctx);
                        request.extensions_mut().insert(user_id);
                        return Ok(next.run(request).await);
                    }
                }

                // Also check against database auth tokens.
                if let Ok(conn) = sf_db::pool::get_conn(&ctx.db) {
                    if let Ok(Some(tok)) = sf_db::queries::auth::get_token(&conn, token) {
                        request.extensions_mut().insert(tok.user_id);
                        return Ok(next.run(request).await);
                    }
                }
            }
        }
    }

    // Check session cookie.
    if let Some(cookie_header) = request.headers().get(axum::http::header::COOKIE) {
        if let Ok(cookies_str) = cookie_header.to_str() {
            for part in cookies_str.split(';') {
                let part = part.trim();
                if let Some(value) = part.strip_prefix(&format!("{SESSION_COOKIE}=")) {
                    // Validate cookie value against database tokens.
                    if let Ok(conn) = sf_db::pool::get_conn(&ctx.db) {
                        if let Ok(Some(tok)) = sf_db::queries::auth::get_token(&conn, value) {
                            request.extensions_mut().insert(tok.user_id);
                            return Ok(next.run(request).await);
                        }
                    }
                }
            }
        }
    }

    Err((StatusCode::UNAUTHORIZED, "Authentication required").into_response())
}

/// Resolve a user ID for API-key authenticated requests.
fn resolve_api_key_user(_ctx: &AppContext) -> UserId {
    ANONYMOUS_USER_ID
        .parse()
        .expect("static anonymous UUID is valid")
}
