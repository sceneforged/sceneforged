//! Playback API routes.
//!
//! These routes handle playback info requests, progress reporting, and user watch data.
//! They provide endpoints for getting playback URLs, reporting progress, and managing
//! favorites and watch status.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use sceneforged_common::{FileRole, ItemId, UserId};
use sceneforged_db::{
    models::{MediaFile, UserItemData},
    queries::{media_files, playback},
};
use serde::{Deserialize, Serialize};

use super::AppContext;

/// Default user ID when auth is disabled
const DEFAULT_USER_ID: &str = "00000000-0000-0000-0000-000000000001";

/// Create playback routes.
pub fn playback_routes() -> Router<AppContext> {
    Router::new()
        // Simplified playback routes (use default user when auth disabled)
        .route("/playback/:item_id/info", get(get_playback_info))
        .route("/playback/:item_id/progress", post(report_progress_simple))
        .route("/playback/:item_id/played", post(mark_played_simple))
        .route("/playback/:item_id/unplayed", post(mark_unplayed_simple))
        .route("/playback/:item_id/favorite", post(toggle_favorite_simple))
        // User-specific routes (for future auth support)
        .route(
            "/users/:user_id/items/:item_id/progress",
            post(report_progress),
        )
        .route("/users/:user_id/items/:item_id/played", post(mark_played))
        .route(
            "/users/:user_id/items/:item_id/played",
            delete(mark_unplayed),
        )
        .route(
            "/users/:user_id/items/:item_id/favorite",
            post(toggle_favorite),
        )
        .route("/users/:user_id/in_progress", get(get_in_progress))
        .route("/users/:user_id/favorites", get(get_favorites))
        .route(
            "/users/:user_id/items/:item_id/user_data",
            get(get_user_item_data),
        )
}

// ============================================================================
// Request/Response types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct PlaybackInfoResponse {
    pub item_id: String,
    pub media_sources: Vec<MediaSourceInfo>,
}

#[derive(Debug, Serialize)]
pub struct MediaSourceInfo {
    pub id: String,
    pub file_path: String,
    pub container: String,
    pub size: i64,
    pub duration_ticks: Option<i64>,
    pub supports_direct_play: bool,
    pub supports_direct_stream: bool,
    pub supports_transcoding: bool,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub is_hdr: bool,
    pub serves_as_universal: bool,
    /// Direct stream URL (for MKV/Profile A)
    pub direct_stream_url: Option<String>,
    /// HLS master playlist URL (for Profile B / universal)
    pub hls_url: Option<String>,
}

impl MediaSourceInfo {
    fn from_media_file(file: MediaFile, base_url: &str) -> Self {
        let direct_stream_url = Some(format!("{}/stream/{}/direct", base_url, file.id));

        let hls_url = if file.serves_as_universal || file.role == FileRole::Universal {
            Some(format!("{}/stream/{}/master.m3u8", base_url, file.id))
        } else {
            None
        };

        Self {
            id: file.id.to_string(),
            file_path: file.file_path,
            container: file.container,
            size: file.file_size,
            duration_ticks: file.duration_ticks,
            supports_direct_play: true,
            supports_direct_stream: true,
            supports_transcoding: false, // Not implementing live transcoding
            video_codec: file.video_codec,
            audio_codec: file.audio_codec,
            width: file.width,
            height: file.height,
            is_hdr: file.is_hdr,
            serves_as_universal: file.serves_as_universal,
            direct_stream_url,
            hls_url,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct PlaybackInfoQuery {
    /// If true, only return web-playable sources (Profile B/universal).
    /// Defaults to false (returns all sources for capable players).
    #[serde(default)]
    pub web_only: bool,
}

#[derive(Debug, Deserialize)]
pub struct ProgressRequest {
    pub position_ticks: i64,
    #[serde(default)]
    pub is_paused: bool,
}

#[derive(Debug, Serialize)]
pub struct UserItemDataResponse {
    pub user_id: String,
    pub item_id: String,
    pub playback_position_ticks: i64,
    pub play_count: i32,
    pub played: bool,
    pub is_favorite: bool,
    pub last_played_date: Option<String>,
}

impl From<UserItemData> for UserItemDataResponse {
    fn from(data: UserItemData) -> Self {
        Self {
            user_id: data.user_id.to_string(),
            item_id: data.item_id.to_string(),
            playback_position_ticks: data.playback_position_ticks,
            play_count: data.play_count,
            played: data.played,
            is_favorite: data.is_favorite,
            last_played_date: data.last_played_date.map(|d| d.to_rfc3339()),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct InProgressItemResponse {
    pub item: super::routes_library::ItemResponse,
    pub user_data: UserItemDataResponse,
}

#[derive(Debug, Deserialize)]
pub struct InProgressQuery {
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    20
}

#[derive(Debug, Serialize)]
pub struct FavoriteResponse {
    pub is_favorite: bool,
}

// ============================================================================
// Handlers
// ============================================================================

/// Get playback info for an item.
async fn get_playback_info(
    State(ctx): State<AppContext>,
    Path(item_id): Path<String>,
    Query(query): Query<PlaybackInfoQuery>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Database not available"})),
        )
            .into_response();
    };

    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    let id = match item_id.parse::<uuid::Uuid>() {
        Ok(uuid) => ItemId::from(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid item ID"})),
            )
                .into_response()
        }
    };

    // Get all media files for this item
    let files = match media_files::list_media_files_for_item(&conn, id) {
        Ok(f) => f,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    if files.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "No media files found for item"})),
        )
            .into_response();
    }

    // Filter sources based on web_only parameter
    let filtered_files: Vec<_> = if query.web_only {
        // Only web-playable sources (universal/serves_as_universal)
        files
            .into_iter()
            .filter(|f| f.serves_as_universal || f.role == FileRole::Universal)
            .collect()
    } else {
        // All sources for capable players like Infuse
        files
    };

    if filtered_files.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "No web-playable sources available. Item needs conversion to Profile B."})),
        )
            .into_response();
    }

    // Build base URL from config
    let base_url = format!(
        "http://{}:{}/api",
        ctx.config.server.host, ctx.config.server.port
    );

    let media_sources: Vec<MediaSourceInfo> = filtered_files
        .into_iter()
        .map(|f| MediaSourceInfo::from_media_file(f, &base_url))
        .collect();

    Json(PlaybackInfoResponse {
        item_id: id.to_string(),
        media_sources,
    })
    .into_response()
}

/// Report playback progress.
async fn report_progress(
    State(ctx): State<AppContext>,
    Path((user_id, item_id)): Path<(String, String)>,
    Json(req): Json<ProgressRequest>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Database not available"})),
        )
            .into_response();
    };

    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    let user = match user_id.parse::<uuid::Uuid>() {
        Ok(uuid) => UserId::from(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid user ID"})),
            )
                .into_response()
        }
    };

    let item = match item_id.parse::<uuid::Uuid>() {
        Ok(uuid) => ItemId::from(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid item ID"})),
            )
                .into_response()
        }
    };

    match playback::update_playback_position(&conn, user, item, req.position_ticks) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Mark an item as played.
async fn mark_played(
    State(ctx): State<AppContext>,
    Path((user_id, item_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Database not available"})),
        )
            .into_response();
    };

    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    let user = match user_id.parse::<uuid::Uuid>() {
        Ok(uuid) => UserId::from(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid user ID"})),
            )
                .into_response()
        }
    };

    let item = match item_id.parse::<uuid::Uuid>() {
        Ok(uuid) => ItemId::from(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid item ID"})),
            )
                .into_response()
        }
    };

    match playback::mark_played(&conn, user, item) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Mark an item as unplayed.
async fn mark_unplayed(
    State(ctx): State<AppContext>,
    Path((user_id, item_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Database not available"})),
        )
            .into_response();
    };

    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    let user = match user_id.parse::<uuid::Uuid>() {
        Ok(uuid) => UserId::from(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid user ID"})),
            )
                .into_response()
        }
    };

    let item = match item_id.parse::<uuid::Uuid>() {
        Ok(uuid) => ItemId::from(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid item ID"})),
            )
                .into_response()
        }
    };

    match playback::mark_unplayed(&conn, user, item) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Toggle favorite status.
async fn toggle_favorite(
    State(ctx): State<AppContext>,
    Path((user_id, item_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Database not available"})),
        )
            .into_response();
    };

    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    let user = match user_id.parse::<uuid::Uuid>() {
        Ok(uuid) => UserId::from(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid user ID"})),
            )
                .into_response()
        }
    };

    let item = match item_id.parse::<uuid::Uuid>() {
        Ok(uuid) => ItemId::from(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid item ID"})),
            )
                .into_response()
        }
    };

    match playback::toggle_favorite(&conn, user, item) {
        Ok(is_favorite) => Json(FavoriteResponse { is_favorite }).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Get in-progress items for a user (continue watching).
async fn get_in_progress(
    State(ctx): State<AppContext>,
    Path(user_id): Path<String>,
    Query(query): Query<InProgressQuery>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Database not available"})),
        )
            .into_response();
    };

    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    let user = match user_id.parse::<uuid::Uuid>() {
        Ok(uuid) => UserId::from(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid user ID"})),
            )
                .into_response()
        }
    };

    match playback::get_in_progress_items(&conn, user, query.limit) {
        Ok(items_with_data) => {
            let response: Vec<InProgressItemResponse> = items_with_data
                .into_iter()
                .map(|(item, data)| InProgressItemResponse {
                    item: item.into(),
                    user_data: data.into(),
                })
                .collect();
            Json(response).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Get favorite items for a user.
async fn get_favorites(
    State(ctx): State<AppContext>,
    Path(user_id): Path<String>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Database not available"})),
        )
            .into_response();
    };

    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    let user = match user_id.parse::<uuid::Uuid>() {
        Ok(uuid) => UserId::from(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid user ID"})),
            )
                .into_response()
        }
    };

    match playback::get_favorites(&conn, user) {
        Ok(fav_items) => {
            let response: Vec<super::routes_library::ItemResponse> =
                fav_items.into_iter().map(Into::into).collect();
            Json(response).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Get user item data.
async fn get_user_item_data(
    State(ctx): State<AppContext>,
    Path((user_id, item_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Database not available"})),
        )
            .into_response();
    };

    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    let user = match user_id.parse::<uuid::Uuid>() {
        Ok(uuid) => UserId::from(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid user ID"})),
            )
                .into_response()
        }
    };

    let item = match item_id.parse::<uuid::Uuid>() {
        Ok(uuid) => ItemId::from(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid item ID"})),
            )
                .into_response()
        }
    };

    match playback::get_user_item_data(&conn, user, item) {
        Ok(data) => Json(UserItemDataResponse::from(data)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ============================================================================
// Simplified handlers (use default user when auth is disabled)
// ============================================================================

/// Report playback progress (simplified - uses default user).
async fn report_progress_simple(
    State(ctx): State<AppContext>,
    Path(item_id): Path<String>,
    Json(req): Json<ProgressRequest>,
) -> impl IntoResponse {
    report_progress(
        State(ctx),
        Path((DEFAULT_USER_ID.to_string(), item_id)),
        Json(req),
    )
    .await
}

/// Mark an item as played (simplified - uses default user).
async fn mark_played_simple(
    State(ctx): State<AppContext>,
    Path(item_id): Path<String>,
) -> impl IntoResponse {
    mark_played(State(ctx), Path((DEFAULT_USER_ID.to_string(), item_id))).await
}

/// Mark an item as unplayed (simplified - uses default user).
async fn mark_unplayed_simple(
    State(ctx): State<AppContext>,
    Path(item_id): Path<String>,
) -> impl IntoResponse {
    mark_unplayed(State(ctx), Path((DEFAULT_USER_ID.to_string(), item_id))).await
}

/// Toggle favorite status (simplified - uses default user).
async fn toggle_favorite_simple(
    State(ctx): State<AppContext>,
    Path(item_id): Path<String>,
) -> impl IntoResponse {
    toggle_favorite(State(ctx), Path((DEFAULT_USER_ID.to_string(), item_id))).await
}
