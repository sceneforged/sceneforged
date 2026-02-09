//! Jellyfin user endpoints (AuthenticateByName, user info).

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::context::AppContext;
use crate::error::AppError;
use crate::middleware::auth::{parse_mediabrowser_header, validate_auth_headers};

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinUser {
    pub id: String,
    pub name: String,
    pub server_id: String,
    pub has_password: bool,
    pub has_configured_password: bool,
    pub has_configured_easy_password: bool,
    pub enable_auto_login: Option<bool>,
    pub policy: UserPolicy,
    pub configuration: UserConfiguration,
}

/// User permissions — Jellyfin clients check these to decide what UI to show.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserPolicy {
    pub is_administrator: bool,
    pub is_disabled: bool,
    pub is_hidden: bool,
    pub enable_remote_access: bool,
    pub enable_media_playback: bool,
    pub enable_audio_playback_transcoding: bool,
    pub enable_video_playback_transcoding: bool,
    pub enable_playback_remuxing: bool,
    pub enable_content_downloading: bool,
    pub enable_all_folders: bool,
    pub enable_all_devices: bool,
    pub enable_all_channels: bool,
}

impl Default for UserPolicy {
    fn default() -> Self {
        Self {
            is_administrator: true,
            is_disabled: false,
            is_hidden: false,
            enable_remote_access: true,
            enable_media_playback: true,
            enable_audio_playback_transcoding: true,
            enable_video_playback_transcoding: true,
            enable_playback_remuxing: true,
            enable_content_downloading: true,
            enable_all_folders: true,
            enable_all_devices: true,
            enable_all_channels: true,
        }
    }
}

/// User configuration for display preferences.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserConfiguration {
    pub play_default_audio_track: bool,
    pub subtitle_mode: String,
    pub enable_next_episode_auto_play: bool,
    pub remember_audio_selections: bool,
    pub remember_subtitle_selections: bool,
}

impl Default for UserConfiguration {
    fn default() -> Self {
        Self {
            play_default_audio_track: true,
            subtitle_mode: "Default".into(),
            enable_next_episode_auto_play: true,
            remember_audio_selections: true,
            remember_subtitle_selections: true,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthResult {
    pub user: JellyfinUser,
    pub access_token: String,
    pub server_id: String,
    pub session_info: SessionInfo,
}

/// Minimal session info — Infuse checks for this in the auth response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SessionInfo {
    pub id: String,
    pub user_id: String,
    pub user_name: String,
    pub client: String,
    pub device_id: String,
    pub device_name: String,
    pub application_version: String,
    pub is_active: bool,
    pub supports_media_control: bool,
    pub supports_remote_control: bool,
    pub playable_media_types: Vec<String>,
    pub server_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthByNameRequest {
    pub username: String,
    pub pw: String,
}

fn make_user(id: String, name: String, is_admin: bool) -> JellyfinUser {
    JellyfinUser {
        id,
        name,
        server_id: "sceneforged-server".into(),
        has_password: true,
        has_configured_password: true,
        has_configured_easy_password: false,
        enable_auto_login: None,
        policy: UserPolicy {
            is_administrator: is_admin,
            ..Default::default()
        },
        configuration: UserConfiguration::default(),
    }
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
            .map(|u| {
                let is_admin = u.role == "admin";
                make_user(u.id.to_string(), u.username, is_admin)
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

    // Parse device info from MediaBrowser header.
    let mb = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(parse_mediabrowser_header);

    let client_name = mb.as_ref().and_then(|m| m.client.clone())
        .unwrap_or_else(|| "Unknown".into());
    let device_name = mb.as_ref().and_then(|m| m.device_name.clone())
        .unwrap_or_else(|| "Unknown".into());
    let device_id = mb.as_ref().and_then(|m| m.device_id.clone())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let app_version = mb.as_ref().and_then(|m| m.version.clone())
        .unwrap_or_default();

    let token = uuid::Uuid::new_v4().to_string();
    let expires = chrono::Utc::now() + chrono::Duration::days(30);
    sf_db::queries::auth::create_token(&conn, user.id, &token, &expires.to_rfc3339())?;

    let is_admin = user.role == "admin";
    let user_id_str = user.id.to_string();
    let username = user.username.clone();

    Ok(Json(AuthResult {
        user: make_user(user_id_str.clone(), username.clone(), is_admin),
        access_token: token,
        server_id: "sceneforged-server".into(),
        session_info: SessionInfo {
            id: uuid::Uuid::new_v4().to_string(),
            user_id: user_id_str,
            user_name: username,
            client: client_name,
            device_id,
            device_name,
            application_version: app_version,
            is_active: true,
            supports_media_control: false,
            supports_remote_control: false,
            playable_media_types: vec!["Video".into(), "Audio".into()],
            server_id: "sceneforged-server".into(),
        },
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

    let is_admin = user.role == "admin";
    Ok(Json(make_user(user.id.to_string(), user.username, is_admin)))
}

/// GET /Users/Me — resolve user from auth token.
///
/// Infuse and other clients call this after login to verify the session
/// is still valid and get fresh user data.
pub async fn get_me(
    State(ctx): State<AppContext>,
    headers: axum::http::HeaderMap,
) -> Result<Json<JellyfinUser>, AppError> {
    let authorization = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    let cookie = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok());

    let x_emby_token = headers
        .get("X-Emby-Token")
        .and_then(|v| v.to_str().ok());

    let user_id = validate_auth_headers(
        &ctx.config.auth,
        &ctx.db,
        authorization,
        cookie,
        x_emby_token,
    )
    .ok_or_else(|| sf_core::Error::Unauthorized("No valid token".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let user = sf_db::queries::users::get_user_by_id(&conn, user_id)?
        .ok_or_else(|| sf_core::Error::not_found("user", user_id))?;

    let is_admin = user.role == "admin";
    Ok(Json(make_user(user.id.to_string(), user.username, is_admin)))
}

/// Jellyfin display preferences — layout settings per user.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct DisplayPreferences {
    pub id: String,
    pub view_type: Option<String>,
    pub sort_by: String,
    pub sort_order: String,
    pub remember_indexing: bool,
    pub remember_sorting: bool,
    pub scroll_direction: String,
    pub custom_prefs: std::collections::HashMap<String, String>,
    pub client: String,
}

/// GET /DisplayPreferences/{id} — return display preferences.
///
/// Jellyfin clients (and Infuse) call this immediately after login.
/// Missing this endpoint is known to block the login flow (Jellyfin issue #6413).
pub async fn display_preferences(
    Path(id): Path<String>,
    Query(params): Query<DisplayPrefsQuery>,
) -> Json<DisplayPreferences> {
    Json(DisplayPreferences {
        id,
        view_type: None,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        remember_indexing: false,
        remember_sorting: false,
        scroll_direction: "Horizontal".into(),
        custom_prefs: std::collections::HashMap::new(),
        client: params.client.unwrap_or_else(|| "emby".into()),
    })
}

#[derive(Debug, Deserialize)]
pub struct DisplayPrefsQuery {
    #[serde(alias = "client", alias = "Client")]
    pub client: Option<String>,
}
