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
///
/// Token resolution order:
/// 1. `Authorization: MediaBrowser ..., Token="<token>"` (Jellyfin clients)
/// 2. `X-Emby-Token: <token>` (Jellyfin shorthand)
/// 3. `Authorization: Bearer <token>` (standard API/web)
/// 4. Cookie: `sceneforged_session=<token>` (web browser)
pub fn validate_auth_headers(
    auth_config: &sf_core::config::AuthConfig,
    db: &DbPool,
    authorization: Option<&str>,
    cookie: Option<&str>,
    x_emby_token: Option<&str>,
) -> Option<UserId> {
    // If auth is not enabled, return anonymous user.
    if !auth_config.enabled {
        return Some(
            ANONYMOUS_USER_ID
                .parse()
                .expect("static anonymous UUID is valid"),
        );
    }

    // 1. Check MediaBrowser Token in Authorization header.
    if let Some(auth_value) = authorization {
        if auth_value.starts_with("MediaBrowser ") || auth_value.starts_with("Emby ") {
            if let Some(token) = extract_mediabrowser_token(auth_value) {
                if let Some(uid) = validate_token(auth_config, db, &token) {
                    return Some(uid);
                }
            }
        }
    }

    // 2. Check X-Emby-Token header (Jellyfin shorthand).
    if let Some(token) = x_emby_token {
        if let Some(uid) = validate_token(auth_config, db, token) {
            return Some(uid);
        }
    }

    // 3. Check Authorization: Bearer header.
    if let Some(auth_value) = authorization {
        if let Some(token) = auth_value.strip_prefix("Bearer ") {
            if let Some(uid) = validate_token(auth_config, db, token) {
                return Some(uid);
            }
        }
    }

    // 4. Check session cookie.
    if let Some(cookies_str) = cookie {
        for part in cookies_str.split(';') {
            let part = part.trim();
            if let Some(value) = part.strip_prefix(&format!("{SESSION_COOKIE}=")) {
                if let Some(uid) = validate_token(auth_config, db, value) {
                    return Some(uid);
                }
            }
        }
    }

    None
}

/// Validate a single token against the config API key and DB tokens.
fn validate_token(
    auth_config: &sf_core::config::AuthConfig,
    db: &DbPool,
    token: &str,
) -> Option<UserId> {
    // Check against config API key.
    if let Some(ref api_key) = auth_config.api_key {
        if token == api_key {
            return Some(
                ANONYMOUS_USER_ID
                    .parse()
                    .expect("static anonymous UUID is valid"),
            );
        }
    }

    // Check against DB tokens.
    if let Ok(conn) = sf_db::pool::get_conn(db) {
        if let Ok(Some(tok)) = sf_db::queries::auth::get_token(&conn, token) {
            return Some(tok.user_id);
        }
    }

    None
}

/// Extract Token value from MediaBrowser/Emby authorization header.
/// Format: `MediaBrowser Client="...", Device="...", Token="<token>"`
pub fn extract_mediabrowser_token(header: &str) -> Option<String> {
    for part in header.split(',') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("Token=") {
            return Some(rest.trim_matches('"').to_string());
        }
    }
    None
}

/// Parse MediaBrowser device info from the Authorization header.
pub fn parse_mediabrowser_header(header: &str) -> MediaBrowserInfo {
    let mut info = MediaBrowserInfo::default();
    for part in header.split(',') {
        let part = part.trim();
        // Strip the "MediaBrowser " or "Emby " prefix from the first field.
        let part = part
            .strip_prefix("MediaBrowser ")
            .or_else(|| part.strip_prefix("Emby "))
            .unwrap_or(part);
        if let Some(val) = part.strip_prefix("Client=") {
            info.client = Some(val.trim_matches('"').to_string());
        } else if let Some(val) = part.strip_prefix("Device=") {
            info.device_name = Some(val.trim_matches('"').to_string());
        } else if let Some(val) = part.strip_prefix("DeviceId=") {
            info.device_id = Some(val.trim_matches('"').to_string());
        } else if let Some(val) = part.strip_prefix("Version=") {
            info.version = Some(val.trim_matches('"').to_string());
        }
    }
    info
}

#[derive(Debug, Default, Clone)]
pub struct MediaBrowserInfo {
    pub client: Option<String>,
    pub device_name: Option<String>,
    pub device_id: Option<String>,
    pub version: Option<String>,
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

    let x_emby_token = request
        .headers()
        .get("X-Emby-Token")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned());

    match validate_auth_headers(
        &ctx.config.auth,
        &ctx.db,
        authorization.as_deref(),
        cookie.as_deref(),
        x_emby_token.as_deref(),
    ) {
        Some(user_id) => {
            request.extensions_mut().insert(user_id);
            Ok(next.run(request).await)
        }
        None => Err((StatusCode::UNAUTHORIZED, "Authentication required").into_response()),
    }
}
