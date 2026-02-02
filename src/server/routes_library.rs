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

use super::AppContext;
use crate::scanner::Scanner;

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

#[derive(Debug, Deserialize)]
pub struct CreateLibraryRequest {
    pub name: String,
    pub media_type: MediaType,
    pub paths: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct LibraryResponse {
    pub id: String,
    pub name: String,
    pub media_type: String,
    pub paths: Vec<String>,
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

#[derive(Debug, Serialize)]
pub struct ItemResponse {
    pub id: String,
    pub library_id: String,
    pub parent_id: Option<String>,
    pub item_kind: String,
    pub name: String,
    pub sort_name: Option<String>,
    pub original_title: Option<String>,
    pub overview: Option<String>,
    pub tagline: Option<String>,
    pub genres: Vec<String>,
    pub studios: Vec<String>,
    pub community_rating: Option<f64>,
    pub production_year: Option<i32>,
    pub premiere_date: Option<String>,
    pub official_rating: Option<String>,
    pub runtime_ticks: Option<i64>,
    pub index_number: Option<i32>,
    pub parent_index_number: Option<i32>,
    pub hdr_type: Option<String>,
    pub dolby_vision_profile: Option<String>,
    pub date_created: String,
    pub provider_ids: ProviderIdsResponse,
}

#[derive(Debug, Serialize)]
pub struct ProviderIdsResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmdb: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imdb: Option<String>,
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

#[derive(Debug, Serialize)]
pub struct MediaFileResponse {
    pub id: String,
    pub item_id: String,
    pub role: String,
    pub file_path: String,
    pub file_size: i64,
    pub container: String,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration_ticks: Option<i64>,
    pub is_hdr: bool,
    pub serves_as_universal: bool,
}

impl From<MediaFile> for MediaFileResponse {
    fn from(file: MediaFile) -> Self {
        Self {
            id: file.id.to_string(),
            item_id: file.item_id.to_string(),
            role: file.role.to_string(),
            file_path: file.file_path,
            file_size: file.file_size,
            container: file.container,
            video_codec: file.video_codec,
            audio_codec: file.audio_codec,
            width: file.width,
            height: file.height,
            duration_ticks: file.duration_ticks,
            is_hdr: file.is_hdr,
            serves_as_universal: file.serves_as_universal,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ItemsQuery {
    pub library_id: Option<String>,
    pub parent_id: Option<String>,
    #[serde(default)]
    pub item_kinds: Option<String>, // Comma-separated
    pub search: Option<String>,
    #[serde(default = "default_offset")]
    pub offset: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub sort_by: Option<String>,
    #[serde(default)]
    pub sort_desc: bool,
}

fn default_offset() -> u32 {
    0
}

fn default_limit() -> u32 {
    100
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(default = "default_search_limit")]
    pub limit: u32,
}

fn default_search_limit() -> u32 {
    20
}

#[derive(Debug, Serialize)]
pub struct ItemsListResponse {
    pub items: Vec<ItemResponse>,
    pub total_count: u32,
    pub offset: u32,
    pub limit: u32,
}

// ============================================================================
// Handlers
// ============================================================================

/// List all libraries.
async fn list_libraries(State(ctx): State<AppContext>) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "Database not available"}))).into_response();
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
async fn create_library(
    State(ctx): State<AppContext>,
    Json(req): Json<CreateLibraryRequest>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "Database not available"}))).into_response();
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
        Ok(lib) => (StatusCode::CREATED, Json(LibraryResponse::from(lib))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Get a library by ID.
async fn get_library(
    State(ctx): State<AppContext>,
    Path(library_id): Path<String>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "Database not available"}))).into_response();
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
async fn delete_library(
    State(ctx): State<AppContext>,
    Path(library_id): Path<String>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "Database not available"}))).into_response();
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
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
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
async fn scan_library(
    State(ctx): State<AppContext>,
    Path(library_id): Path<String>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "Database not available"}))).into_response();
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

    let scanner = Scanner::new(pool.clone());
    match scanner.scan_library(id) {
        Ok(results) => {
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
async fn get_library_items(
    State(ctx): State<AppContext>,
    Path(library_id): Path<String>,
    Query(query): Query<ItemsQuery>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "Database not available"}))).into_response();
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
        parent_id: query.parent_id.as_ref().and_then(|s| {
            s.parse::<uuid::Uuid>()
                .ok()
                .map(ItemId::from)
        }),
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
async fn get_recent_items(
    State(ctx): State<AppContext>,
    Path(library_id): Path<String>,
    Query(query): Query<ItemsQuery>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "Database not available"}))).into_response();
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

/// List all items with filtering.
async fn list_items_handler(
    State(ctx): State<AppContext>,
    Query(query): Query<ItemsQuery>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "Database not available"}))).into_response();
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

    let filter = items::ItemFilter {
        library_id: query.library_id.as_ref().and_then(|s| {
            s.parse::<uuid::Uuid>()
                .ok()
                .map(LibraryId::from)
        }),
        parent_id: query.parent_id.as_ref().and_then(|s| {
            s.parse::<uuid::Uuid>()
                .ok()
                .map(ItemId::from)
        }),
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

/// Get an item by ID.
async fn get_item(
    State(ctx): State<AppContext>,
    Path(item_id): Path<String>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "Database not available"}))).into_response();
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
async fn get_children(
    State(ctx): State<AppContext>,
    Path(item_id): Path<String>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "Database not available"}))).into_response();
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
async fn get_item_files(
    State(ctx): State<AppContext>,
    Path(item_id): Path<String>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "Database not available"}))).into_response();
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
async fn get_similar_items(
    State(ctx): State<AppContext>,
    Path(item_id): Path<String>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "Database not available"}))).into_response();
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
async fn search_items(
    State(ctx): State<AppContext>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "Database not available"}))).into_response();
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
