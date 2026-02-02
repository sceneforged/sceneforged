//! Authentication and authorization middleware for the API and web UI.

use crate::config::AuthConfig;
use crate::server::AppContext;
use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
    Json,
};
use axum_extra::{
    extract::cookie::{Cookie, CookieJar},
    headers::{authorization::Bearer, Authorization},
    typed_header::TypedHeader,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

const SESSION_COOKIE_NAME: &str = "sceneforged_session";

/// Login request payload
#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Login response
#[derive(Serialize)]
pub struct LoginResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
}

/// Session data stored in the cookie
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionData {
    username: String,
    expires_at: u64,
}

impl SessionData {
    fn new(username: &str, timeout_hours: u64) -> Self {
        let expires_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + (timeout_hours * 3600);
        Self {
            username: username.to_string(),
            expires_at,
        }
    }

    fn is_valid(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now < self.expires_at
    }

    fn encode(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        STANDARD.encode(json)
    }

    fn decode(encoded: &str) -> Option<Self> {
        let json = STANDARD.decode(encoded).ok()?;
        serde_json::from_slice(&json).ok()
    }
}

/// Check if authentication is required and valid
fn check_auth(
    auth_config: &AuthConfig,
    bearer_token: Option<&str>,
    session_cookie: Option<&str>,
) -> Result<(), (StatusCode, &'static str)> {
    if !auth_config.enabled {
        return Ok(());
    }

    // Check API key first (for programmatic access)
    if let Some(token) = bearer_token {
        if let Some(ref api_key) = auth_config.api_key {
            if token == api_key {
                return Ok(());
            }
        }
    }

    // Check session cookie (for web UI)
    if let Some(cookie_value) = session_cookie {
        if let Some(session) = SessionData::decode(cookie_value) {
            if session.is_valid() {
                return Ok(());
            }
        }
    }

    Err((StatusCode::UNAUTHORIZED, "Authentication required"))
}

/// Middleware for API key authentication
pub async fn api_auth_middleware(
    State(ctx): State<AppContext>,
    bearer: Option<TypedHeader<Authorization<Bearer>>>,
    jar: CookieJar,
    request: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, &'static str)> {
    let bearer_token = bearer.map(|b| b.token().to_string());
    let session_cookie = jar.get(SESSION_COOKIE_NAME).map(|c| c.value().to_string());

    check_auth(
        &ctx.config.server.auth,
        bearer_token.as_deref(),
        session_cookie.as_deref(),
    )?;

    Ok(next.run(request).await)
}

/// Login handler
pub async fn login(
    State(ctx): State<AppContext>,
    jar: CookieJar,
    Json(payload): Json<LoginRequest>,
) -> Result<(CookieJar, Json<LoginResponse>), (StatusCode, Json<LoginResponse>)> {
    let auth_config = &ctx.config.server.auth;

    // Check if auth is configured
    let (expected_username, password_hash) =
        match (&auth_config.username, &auth_config.password_hash) {
            (Some(u), Some(h)) => (u, h),
            _ => {
                return Err((
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(LoginResponse {
                        success: false,
                        message: "Authentication not configured".to_string(),
                        expires_at: None,
                    }),
                ));
            }
        };

    // Verify username
    if payload.username != *expected_username {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(LoginResponse {
                success: false,
                message: "Invalid credentials".to_string(),
                expires_at: None,
            }),
        ));
    }

    // Verify password
    match bcrypt::verify(&payload.password, password_hash) {
        Ok(true) => {
            // Create session
            let session = SessionData::new(&payload.username, auth_config.session_timeout_hours);
            let expires_at = session.expires_at;

            // Create cookie
            let cookie = Cookie::build((SESSION_COOKIE_NAME, session.encode()))
                .path("/")
                .http_only(true)
                .same_site(axum_extra::extract::cookie::SameSite::Lax)
                .max_age(time::Duration::hours(
                    auth_config.session_timeout_hours as i64,
                ))
                .build();

            Ok((
                jar.add(cookie),
                Json(LoginResponse {
                    success: true,
                    message: "Login successful".to_string(),
                    expires_at: Some(expires_at),
                }),
            ))
        }
        Ok(false) | Err(_) => Err((
            StatusCode::UNAUTHORIZED,
            Json(LoginResponse {
                success: false,
                message: "Invalid credentials".to_string(),
                expires_at: None,
            }),
        )),
    }
}

/// Logout handler
pub async fn logout(jar: CookieJar) -> (CookieJar, StatusCode) {
    let cookie = Cookie::build((SESSION_COOKIE_NAME, ""))
        .path("/")
        .max_age(time::Duration::ZERO)
        .build();

    (jar.remove(cookie), StatusCode::OK)
}

/// Check current auth status
pub async fn auth_status(
    State(ctx): State<AppContext>,
    bearer: Option<TypedHeader<Authorization<Bearer>>>,
    jar: CookieJar,
) -> Json<AuthStatusResponse> {
    let auth_config = &ctx.config.server.auth;

    if !auth_config.enabled {
        return Json(AuthStatusResponse {
            auth_enabled: false,
            authenticated: true,
            username: None,
        });
    }

    let bearer_token = bearer.map(|b| b.token().to_string());
    let session_cookie = jar.get(SESSION_COOKIE_NAME).map(|c| c.value().to_string());

    let authenticated = check_auth(
        auth_config,
        bearer_token.as_deref(),
        session_cookie.as_deref(),
    )
    .is_ok();

    let username = session_cookie
        .and_then(|c| SessionData::decode(&c))
        .filter(|s| s.is_valid())
        .map(|s| s.username);

    Json(AuthStatusResponse {
        auth_enabled: true,
        authenticated,
        username,
    })
}

#[derive(Serialize)]
pub struct AuthStatusResponse {
    pub auth_enabled: bool,
    pub authenticated: bool,
    pub username: Option<String>,
}

/// Generate a bcrypt password hash
pub fn hash_password(password: &str) -> Result<String, bcrypt::BcryptError> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST)
}

/// Generate a random API key
pub fn generate_api_key() -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Generate a random webhook signature secret
pub fn generate_secret() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    hex::encode(bytes)
}

/// Verify webhook signature
pub fn verify_webhook_signature(secret: &str, body: &[u8], signature: &str) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };

    mac.update(body);

    // Signature format: sha256=<hex>
    let expected_sig = if let Some(hex_sig) = signature.strip_prefix("sha256=") {
        hex_sig
    } else {
        signature
    };

    let expected_bytes = match hex::decode(expected_sig) {
        Ok(b) => b,
        Err(_) => return false,
    };

    mac.verify_slice(&expected_bytes).is_ok()
}
