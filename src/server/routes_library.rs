//! Library and item API routes.
//!
//! These routes provide Jellyfin-compatible endpoints for browsing and managing
//! the media library. They include endpoints for libraries, items, and search.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use sceneforged_common::{ItemId, LibraryId, MediaType};
use sceneforged_db::{
    models::{Item, Library, MediaFile},
    queries::{items, libraries, media_files},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::AppContext;
use crate::scanner::Scanner;
use crate::state::AppEvent;

/// Create library routes.
pub fn library_routes() -> Router<AppContext> {
    Router::new()
        // Library management
        .route("/libraries", get(list_libraries).post(create_library))
        .route(
            "/libraries/:library_id",
            get(get_library).delete(delete_library),
        )
        .route("/libraries/:library_id/scan", post(scan_library))
        .route("/libraries/:library_id/items", get(get_library_items))
        .route("/libraries/:library_id/recent", get(get_recent_items))
        // Item management
        .route("/items", get(list_items_handler))
        .route("/items/:item_id", get(get_item))
        .route("/items/:item_id/children", get(get_children))
        .route("/items/:item_id/files", get(get_item_files))
        .route("/items/:item_id/similar", get(get_similar_items))
        // Search
        .route("/search", get(search_items))
}

// ============================================================================
// Request/Response types
// ============================================================================

/// Request to create a new library.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateLibraryRequest {
    /// Library name
    pub name: String,
    /// Type of media in this library (movie, series)
    #[schema(value_type = String)]
    pub media_type: MediaType,
    /// Paths to scan for media files
    pub paths: Vec<String>,
}

/// Library information.
#[derive(Debug, Serialize, ToSchema)]
pub struct LibraryResponse {
    /// Unique library identifier
    pub id: String,
    /// Library name
    pub name: String,
    /// Type of media in this library
    pub media_type: String,
    /// Paths scanned for media files
    pub paths: Vec<String>,
    /// When the library was created
    pub created_at: String,
}

impl From<Library> for LibraryResponse {
    fn from(lib: Library) -> Self {
        Self {
            id: lib.id.to_string(),
            name: lib.name,
            media_type: lib.media_type.to_string(),
            paths: lib.paths,
            created_at: lib.created_at.to_rfc3339(),
        }
    }
}

/// Media item information.
#[derive(Debug, Serialize, ToSchema)]
pub struct ItemResponse {
    /// Unique item identifier
    pub id: String,
    /// Library this item belongs to
    pub library_id: String,
    /// Parent item ID (for episodes, seasons, etc.)
    pub parent_id: Option<String>,
    /// Type of item (movie, series, season, episode)
    pub item_kind: String,
    /// Item name/title
    pub name: String,
    /// Sort name
    pub sort_name: Option<String>,
    /// Original title (if different from name)
    pub original_title: Option<String>,
    /// Plot summary
    pub overview: Option<String>,
    /// Tagline
    pub tagline: Option<String>,
    /// Genres
    pub genres: Vec<String>,
    /// Studios
    pub studios: Vec<String>,
    /// Community rating
    pub community_rating: Option<f64>,
    /// Year of production
    pub production_year: Option<i32>,
    /// Premiere date
    pub premiere_date: Option<String>,
    /// Content rating (PG, R, etc.)
    pub official_rating: Option<String>,
    /// Runtime in ticks (100ns units)
    pub runtime_ticks: Option<i64>,
    /// Index number (episode number, etc.)
    pub index_number: Option<i32>,
    /// Parent index number (season number, etc.)
    pub parent_index_number: Option<i32>,
    /// HDR format type
    pub hdr_type: Option<String>,
    /// Dolby Vision profile
    pub dolby_vision_profile: Option<String>,
    /// When the item was added
    pub date_created: String,
    /// External provider IDs
    pub provider_ids: ProviderIdsResponse,
}

/// External provider IDs.
#[derive(Debug, Serialize, ToSchema)]
pub struct ProviderIdsResponse {
    /// TMDB ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmdb: Option<String>,
    /// IMDB ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imdb: Option<String>,
    /// TVDB ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tvdb: Option<String>,
}

impl From<Item> for ItemResponse {
    fn from(item: Item) -> Self {
        Self {
            id: item.id.to_string(),
            library_id: item.library_id.to_string(),
            parent_id: item.parent_id.map(|id| id.to_string()),
            item_kind: item.item_kind.to_string(),
            name: item.name,
            sort_name: item.sort_name,
            original_title: item.original_title,
            overview: item.overview,
            tagline: item.tagline,
            genres: item.genres,
            studios: item.studios,
            community_rating: item.community_rating,
            production_year: item.production_year,
            premiere_date: item.premiere_date,
            official_rating: item.official_rating,
            runtime_ticks: item.runtime_ticks,
            index_number: item.index_number,
            parent_index_number: item.parent_index_number,
            hdr_type: item.hdr_type,
            dolby_vision_profile: item.dolby_vision_profile,
            date_created: item.date_created.to_rfc3339(),
            provider_ids: ProviderIdsResponse {
                tmdb: item.provider_ids.tmdb,
                imdb: item.provider_ids.imdb,
                tvdb: item.provider_ids.tvdb,
            },
        }
    }
}

/// Media file information.
#[derive(Debug, Serialize, ToSchema)]
pub struct MediaFileResponse {
    /// Unique file identifier
    pub id: String,
    /// Item this file belongs to
    pub item_id: String,
    /// File role (source, universal, extra)
    pub role: String,
    /// File profile (A, B, C)
    pub profile: String,
    /// Whether this file can be classified as Profile A
    pub can_be_profile_a: bool,
    /// Whether this file can be converted to Profile B
    pub can_be_profile_b: bool,
    /// Path to the file
    pub file_path: String,
    /// File size in bytes
    pub file_size: i64,
    /// Container format
    pub container: String,
    /// Video codec
    pub video_codec: Option<String>,
    /// Audio codec
    pub audio_codec: Option<String>,
    /// Video width in pixels
    pub width: Option<i32>,
    /// Video height in pixels
    pub height: Option<i32>,
    /// Duration in ticks (100ns units)
    pub duration_ticks: Option<i64>,
    /// Bit rate in bits per second
    pub bit_rate: Option<i64>,
    /// Whether the file is HDR
    pub is_hdr: bool,
    /// Whether this file serves as universal fallback
    pub serves_as_universal: bool,
    /// Whether this file has faststart moov atom
    pub has_faststart: bool,
    /// Keyframe interval in seconds
    pub keyframe_interval_secs: Option<f64>,
    /// File creation timestamp
    pub created_at: String,
}

impl From<MediaFile> for MediaFileResponse {
    fn from(file: MediaFile) -> Self {
        Self {
            id: file.id.to_string(),
            item_id: file.item_id.to_string(),
            role: file.role.to_string(),
            profile: file.profile.to_string(),
            can_be_profile_a: file.can_be_profile_a,
            can_be_profile_b: file.can_be_profile_b,
            file_path: file.file_path,
            file_size: file.file_size,
            container: file.container,
            video_codec: file.video_codec,
            audio_codec: file.audio_codec,
            width: file.width,
            height: file.height,
            duration_ticks: file.duration_ticks,
            bit_rate: file.bit_rate,
            is_hdr: file.is_hdr,
            serves_as_universal: file.serves_as_universal,
            has_faststart: file.has_faststart,
            keyframe_interval_secs: file.keyframe_interval_secs,
            created_at: file.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ItemsQuery {
    /// Filter by library ID
    pub library_id: Option<String>,
    /// Filter by parent item ID
    pub parent_id: Option<String>,
    /// Filter by item kinds (comma-separated)
    #[serde(default)]
    pub item_kinds: Option<String>,
    /// Search term
    pub search: Option<String>,
    /// Number of results to skip
    #[serde(default = "default_offset")]
    pub offset: u32,
    /// Maximum number of results to return
    #[serde(default = "default_limit")]
    pub limit: u32,
    /// Sort field
    #[serde(default)]
    pub sort_by: Option<String>,
    /// Sort in descending order
    #[serde(default)]
    pub sort_desc: bool,
    /// Special filter: continue_watching, recently_added, favorites
    #[serde(default)]
    pub filter: Option<String>,
    /// User ID for user-specific filters (defaults to default user)
    #[serde(default)]
    pub user_id: Option<String>,
}

fn default_offset() -> u32 {
    0
}

fn default_limit() -> u32 {
    100
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct SearchQuery {
    /// Search term
    pub q: String,
    /// Maximum number of results to return
    #[serde(default = "default_search_limit")]
    pub limit: u32,
}

fn default_search_limit() -> u32 {
    20
}

/// Paginated list of items.
#[derive(Debug, Serialize, ToSchema)]
pub struct ItemsListResponse {
    /// Items in this page
    pub items: Vec<ItemResponse>,
    /// Total number of items matching the filter
    pub total_count: u32,
    /// Offset used
    pub offset: u32,
    /// Limit used
    pub limit: u32,
}

// ============================================================================
// Handlers
// ============================================================================

/// List all libraries.
#[utoipa::path(
    get,
    path = "/api/libraries",
    tag = "libraries",
    responses(
        (status = 200, description = "List of libraries", body = Vec<LibraryResponse>),
        (status = 503, description = "Database not available")
    )
)]
pub async fn list_libraries(State(ctx): State<AppContext>) -> impl IntoResponse {
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

    match libraries::list_libraries(&conn) {
        Ok(libs) => {
            let response: Vec<LibraryResponse> = libs.into_iter().map(Into::into).collect();
            Json(response).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Create a new library.
#[utoipa::path(
    post,
    path = "/api/libraries",
    tag = "libraries",
    request_body = CreateLibraryRequest,
    responses(
        (status = 201, description = "Library created", body = LibraryResponse),
        (status = 500, description = "Internal server error"),
        (status = 503, description = "Database not available")
    )
)]
pub async fn create_library(
    State(ctx): State<AppContext>,
    Json(req): Json<CreateLibraryRequest>,
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

    match libraries::create_library(&conn, &req.name, req.media_type, &req.paths) {
        Ok(lib) => {
            // Broadcast library created event
            ctx.state.broadcast(AppEvent::library_created(lib.clone()));
            (StatusCode::CREATED, Json(LibraryResponse::from(lib))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Get a library by ID.
#[utoipa::path(
    get,
    path = "/api/libraries/{library_id}",
    tag = "libraries",
    params(
        ("library_id" = String, Path, description = "Library ID")
    ),
    responses(
        (status = 200, description = "Library details", body = LibraryResponse),
        (status = 400, description = "Invalid library ID"),
        (status = 404, description = "Library not found"),
        (status = 503, description = "Database not available")
    )
)]
pub async fn get_library(
    State(ctx): State<AppContext>,
    Path(library_id): Path<String>,
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

    let id = match library_id.parse::<uuid::Uuid>() {
        Ok(uuid) => LibraryId::from(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid library ID"})),
            )
                .into_response()
        }
    };

    match libraries::get_library(&conn, id) {
        Ok(Some(lib)) => Json(LibraryResponse::from(lib)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Library not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Delete a library.
#[utoipa::path(
    delete,
    path = "/api/libraries/{library_id}",
    tag = "libraries",
    params(
        ("library_id" = String, Path, description = "Library ID")
    ),
    responses(
        (status = 204, description = "Library deleted"),
        (status = 400, description = "Invalid library ID"),
        (status = 404, description = "Library not found"),
        (status = 503, description = "Database not available")
    )
)]
pub async fn delete_library(
    State(ctx): State<AppContext>,
    Path(library_id): Path<String>,
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

    let id = match library_id.parse::<uuid::Uuid>() {
        Ok(uuid) => LibraryId::from(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid library ID"})),
            )
                .into_response()
        }
    };

    match libraries::delete_library(&conn, id) {
        Ok(true) => {
            // Broadcast library deleted event
            ctx.state.broadcast(AppEvent::library_deleted(library_id));
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Library not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Scan a library for new media files.
#[utoipa::path(
    post,
    path = "/api/libraries/{library_id}/scan",
    tag = "libraries",
    params(
        ("library_id" = String, Path, description = "Library ID")
    ),
    responses(
        (status = 200, description = "Scan completed"),
        (status = 400, description = "Invalid library ID"),
        (status = 500, description = "Scan failed"),
        (status = 503, description = "Database not available")
    )
)]
pub async fn scan_library(
    State(ctx): State<AppContext>,
    Path(library_id): Path<String>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Database not available"})),
        )
            .into_response();
    };

    let id = match library_id.parse::<uuid::Uuid>() {
        Ok(uuid) => LibraryId::from(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid library ID"})),
            )
                .into_response()
        }
    };

    // Broadcast scan started event
    ctx.state
        .broadcast(AppEvent::library_scan_started(library_id.clone()));

    let scanner = Scanner::new(pool.clone(), ctx.config.clone());
    match scanner.scan_library(id) {
        Ok(results) => {
            let items_added = results.len() as u32;

            // Broadcast ItemAdded and PlaybackAvailable events for each item
            for result in &results {
                ctx.state.broadcast(AppEvent::item_added(result.item.clone()));

                // If the source serves as universal, playback is immediately available
                if result.serves_as_universal {
                    ctx.state
                        .broadcast(AppEvent::playback_available(result.item_id.to_string()));
                }
            }

            // Broadcast scan complete event
            ctx.state
                .broadcast(AppEvent::library_scan_complete(library_id, items_added));

            let response = serde_json::json!({
                "files_scanned": results.len(),
                "files_added": results.len(),
            });
            Json(response).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Get items in a library.
#[utoipa::path(
    get,
    path = "/api/libraries/{library_id}/items",
    tag = "libraries",
    params(
        ("library_id" = String, Path, description = "Library ID"),
        ItemsQuery
    ),
    responses(
        (status = 200, description = "List of items", body = ItemsListResponse),
        (status = 400, description = "Invalid library ID"),
        (status = 503, description = "Database not available")
    )
)]
pub async fn get_library_items(
    State(ctx): State<AppContext>,
    Path(library_id): Path<String>,
    Query(query): Query<ItemsQuery>,
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

    let lib_id = match library_id.parse::<uuid::Uuid>() {
        Ok(uuid) => LibraryId::from(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid library ID"})),
            )
                .into_response()
        }
    };

    let filter = items::ItemFilter {
        library_id: Some(lib_id),
        parent_id: query
            .parent_id
            .as_ref()
            .and_then(|s| s.parse::<uuid::Uuid>().ok().map(ItemId::from)),
        item_kinds: query.item_kinds.as_ref().map(|s| {
            s.split(',')
                .filter_map(|k| serde_json::from_str(&format!("\"{}\"", k)).ok())
                .collect()
        }),
        search_term: query.search.clone(),
        ..Default::default()
    };

    let sort = build_sort_options(&query);
    let pagination = items::Pagination {
        offset: query.offset,
        limit: query.limit,
    };

    let total = match items::count_items(&conn, &filter) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    match items::list_items(&conn, &filter, &sort, &pagination) {
        Ok(items_list) => Json(ItemsListResponse {
            items: items_list.into_iter().map(Into::into).collect(),
            total_count: total,
            offset: query.offset,
            limit: query.limit,
        })
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Get recently added items in a library.
#[utoipa::path(
    get,
    path = "/api/libraries/{library_id}/recent",
    tag = "libraries",
    params(
        ("library_id" = String, Path, description = "Library ID"),
        ItemsQuery
    ),
    responses(
        (status = 200, description = "List of recent items", body = Vec<ItemResponse>),
        (status = 400, description = "Invalid library ID"),
        (status = 503, description = "Database not available")
    )
)]
pub async fn get_recent_items(
    State(ctx): State<AppContext>,
    Path(library_id): Path<String>,
    Query(query): Query<ItemsQuery>,
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

    let lib_id = match library_id.parse::<uuid::Uuid>() {
        Ok(uuid) => Some(LibraryId::from(uuid)),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid library ID"})),
            )
                .into_response()
        }
    };

    match items::get_recent_items(&conn, lib_id, query.limit) {
        Ok(items_list) => {
            let response: Vec<ItemResponse> = items_list.into_iter().map(Into::into).collect();
            Json(response).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Default user ID when auth is disabled
const DEFAULT_USER_ID: &str = "00000000-0000-0000-0000-000000000001";

/// List all items with filtering.
///
/// Supports special filters via the `filter` query param:
/// - `continue_watching`: Items with playback progress (position > 0, not marked played)
/// - `recently_added`: Items created in the last 7 days
/// - `favorites`: Items marked as favorite by the user
#[utoipa::path(
    get,
    path = "/api/items",
    tag = "items",
    params(ItemsQuery),
    responses(
        (status = 200, description = "List of items", body = ItemsListResponse),
        (status = 400, description = "Invalid filter"),
        (status = 503, description = "Database not available")
    )
)]
pub async fn list_items_handler(
    State(ctx): State<AppContext>,
    Query(query): Query<ItemsQuery>,
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

    let pagination = items::Pagination {
        offset: query.offset,
        limit: query.limit,
    };

    // Handle special filters
    if let Some(ref filter_name) = query.filter {
        return handle_special_filter(&conn, filter_name, &query, &pagination);
    }

    // Standard item listing
    let filter = items::ItemFilter {
        library_id: query
            .library_id
            .as_ref()
            .and_then(|s| s.parse::<uuid::Uuid>().ok().map(LibraryId::from)),
        parent_id: query
            .parent_id
            .as_ref()
            .and_then(|s| s.parse::<uuid::Uuid>().ok().map(ItemId::from)),
        item_kinds: query.item_kinds.as_ref().map(|s| {
            s.split(',')
                .filter_map(|k| serde_json::from_str(&format!("\"{}\"", k)).ok())
                .collect()
        }),
        search_term: query.search.clone(),
        ..Default::default()
    };

    let sort = build_sort_options(&query);

    let total = match items::count_items(&conn, &filter) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    match items::list_items(&conn, &filter, &sort, &pagination) {
        Ok(items_list) => Json(ItemsListResponse {
            items: items_list.into_iter().map(Into::into).collect(),
            total_count: total,
            offset: pagination.offset,
            limit: pagination.limit,
        })
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Handle special filter types (continue_watching, recently_added, favorites).
fn handle_special_filter(
    conn: &rusqlite::Connection,
    filter_name: &str,
    query: &ItemsQuery,
    pagination: &items::Pagination,
) -> axum::response::Response {
    use sceneforged_common::UserId;

    match filter_name {
        "continue_watching" => {
            // Get user ID from query or use default
            let user_id = query
                .user_id
                .as_ref()
                .and_then(|s| s.parse::<uuid::Uuid>().ok())
                .map(UserId::from)
                .unwrap_or_else(|| UserId::from(uuid::Uuid::parse_str(DEFAULT_USER_ID).unwrap()));

            let total = match items::count_continue_watching_items(conn, user_id) {
                Ok(c) => c,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": e.to_string()})),
                    )
                        .into_response()
                }
            };

            match items::get_continue_watching_items(conn, user_id, pagination) {
                Ok(items_list) => Json(ItemsListResponse {
                    items: items_list.into_iter().map(Into::into).collect(),
                    total_count: total,
                    offset: pagination.offset,
                    limit: pagination.limit,
                })
                .into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
                    .into_response(),
            }
        }
        "recently_added" => {
            // Items added in the last 7 days
            const RECENTLY_ADDED_DAYS: u32 = 7;

            let total = match items::count_recently_added_items(conn, RECENTLY_ADDED_DAYS) {
                Ok(c) => c,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": e.to_string()})),
                    )
                        .into_response()
                }
            };

            match items::get_recently_added_items(conn, RECENTLY_ADDED_DAYS, pagination) {
                Ok(items_list) => Json(ItemsListResponse {
                    items: items_list.into_iter().map(Into::into).collect(),
                    total_count: total,
                    offset: pagination.offset,
                    limit: pagination.limit,
                })
                .into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
                    .into_response(),
            }
        }
        "favorites" => {
            // Get user ID from query or use default
            let user_id = query
                .user_id
                .as_ref()
                .and_then(|s| s.parse::<uuid::Uuid>().ok())
                .map(UserId::from)
                .unwrap_or_else(|| UserId::from(uuid::Uuid::parse_str(DEFAULT_USER_ID).unwrap()));

            let total = match items::count_favorite_items(conn, user_id) {
                Ok(c) => c,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": e.to_string()})),
                    )
                        .into_response()
                }
            };

            match items::get_favorite_items(conn, user_id, pagination) {
                Ok(items_list) => Json(ItemsListResponse {
                    items: items_list.into_iter().map(Into::into).collect(),
                    total_count: total,
                    offset: pagination.offset,
                    limit: pagination.limit,
                })
                .into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
                    .into_response(),
            }
        }
        _ => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Unknown filter: {}. Valid filters: continue_watching, recently_added, favorites", filter_name)
            })),
        )
            .into_response(),
    }
}

/// Get an item by ID.
#[utoipa::path(
    get,
    path = "/api/items/{item_id}",
    tag = "items",
    params(
        ("item_id" = String, Path, description = "Item ID")
    ),
    responses(
        (status = 200, description = "Item details", body = ItemResponse),
        (status = 400, description = "Invalid item ID"),
        (status = 404, description = "Item not found"),
        (status = 503, description = "Database not available")
    )
)]
pub async fn get_item(
    State(ctx): State<AppContext>,
    Path(item_id): Path<String>,
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

    match items::get_item(&conn, id) {
        Ok(Some(item)) => Json(ItemResponse::from(item)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Item not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Get children of an item.
#[utoipa::path(
    get,
    path = "/api/items/{item_id}/children",
    tag = "items",
    params(
        ("item_id" = String, Path, description = "Item ID")
    ),
    responses(
        (status = 200, description = "List of child items", body = Vec<ItemResponse>),
        (status = 400, description = "Invalid item ID"),
        (status = 503, description = "Database not available")
    )
)]
pub async fn get_children(
    State(ctx): State<AppContext>,
    Path(item_id): Path<String>,
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

    match items::get_children(&conn, id) {
        Ok(children) => {
            let response: Vec<ItemResponse> = children.into_iter().map(Into::into).collect();
            Json(response).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Get media files for an item.
#[utoipa::path(
    get,
    path = "/api/items/{item_id}/files",
    tag = "items",
    params(
        ("item_id" = String, Path, description = "Item ID")
    ),
    responses(
        (status = 200, description = "List of media files", body = Vec<MediaFileResponse>),
        (status = 400, description = "Invalid item ID"),
        (status = 503, description = "Database not available")
    )
)]
pub async fn get_item_files(
    State(ctx): State<AppContext>,
    Path(item_id): Path<String>,
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

    match media_files::list_media_files_for_item(&conn, id) {
        Ok(files) => {
            let response: Vec<MediaFileResponse> = files.into_iter().map(Into::into).collect();
            Json(response).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Get similar items (placeholder - returns items from same genre/library).
#[utoipa::path(
    get,
    path = "/api/items/{item_id}/similar",
    tag = "items",
    params(
        ("item_id" = String, Path, description = "Item ID")
    ),
    responses(
        (status = 200, description = "List of similar items", body = Vec<ItemResponse>),
        (status = 400, description = "Invalid item ID"),
        (status = 404, description = "Item not found"),
        (status = 503, description = "Database not available")
    )
)]
pub async fn get_similar_items(
    State(ctx): State<AppContext>,
    Path(item_id): Path<String>,
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

    // Get the item to find its library
    let item = match items::get_item(&conn, id) {
        Ok(Some(item)) => item,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Item not found"})),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    // Get random items from same library (simplified similar items)
    let filter = items::ItemFilter {
        library_id: Some(item.library_id),
        item_kinds: Some(vec![item.item_kind]),
        ..Default::default()
    };
    let sort = items::SortOptions {
        field: items::SortField::Random,
        descending: false,
    };
    let pagination = items::Pagination {
        offset: 0,
        limit: 10,
    };

    match items::list_items(&conn, &filter, &sort, &pagination) {
        Ok(similar) => {
            // Filter out the original item
            let response: Vec<ItemResponse> = similar
                .into_iter()
                .filter(|i| i.id != id)
                .map(Into::into)
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

/// Search items.
#[utoipa::path(
    get,
    path = "/api/search",
    tag = "items",
    params(SearchQuery),
    responses(
        (status = 200, description = "Search results", body = Vec<ItemResponse>),
        (status = 503, description = "Database not available")
    )
)]
pub async fn search_items(
    State(ctx): State<AppContext>,
    Query(query): Query<SearchQuery>,
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

    match items::search_items(&conn, &query.q, query.limit) {
        Ok(results) => {
            let response: Vec<ItemResponse> = results.into_iter().map(Into::into).collect();
            Json(response).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn build_sort_options(query: &ItemsQuery) -> items::SortOptions {
    let field = match query.sort_by.as_deref() {
        Some("name") => items::SortField::Name,
        Some("date_created") => items::SortField::DateCreated,
        Some("date_modified") => items::SortField::DateModified,
        Some("premiere_date") => items::SortField::PremiereDate,
        Some("production_year") => items::SortField::ProductionYear,
        Some("community_rating") => items::SortField::CommunityRating,
        Some("random") => items::SortField::Random,
        _ => items::SortField::Name,
    };

    items::SortOptions {
        field,
        descending: query.sort_desc,
    }
}
