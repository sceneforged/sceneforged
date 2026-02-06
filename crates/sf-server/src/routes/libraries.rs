//! Library CRUD route handlers.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::context::AppContext;
use crate::error::AppError;
use crate::routes::items::ItemResponse;

/// Request body for creating a library.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateLibraryRequest {
    pub name: String,
    pub media_type: String,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    #[schema(value_type = Object)]
    pub config: serde_json::Value,
}

/// Library response.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct LibraryResponse {
    pub id: String,
    pub name: String,
    pub media_type: String,
    pub paths: Vec<String>,
    #[schema(value_type = Object)]
    pub config: serde_json::Value,
    pub created_at: String,
}

impl LibraryResponse {
    fn from_model(lib: &sf_db::models::Library) -> Self {
        Self {
            id: lib.id.to_string(),
            name: lib.name.clone(),
            media_type: lib.media_type.clone(),
            paths: lib.paths.clone(),
            config: lib.config.clone(),
            created_at: lib.created_at.clone(),
        }
    }
}

/// GET /api/libraries
#[utoipa::path(
    get,
    path = "/api/libraries",
    responses(
        (status = 200, description = "List all libraries", body = Vec<LibraryResponse>)
    )
)]
pub async fn list_libraries(
    State(ctx): State<AppContext>,
) -> Result<Json<Vec<LibraryResponse>>, AppError> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let libs = sf_db::queries::libraries::list_libraries(&conn)?;
    let responses: Vec<LibraryResponse> = libs.iter().map(LibraryResponse::from_model).collect();
    Ok(Json(responses))
}

/// POST /api/libraries
#[utoipa::path(
    post,
    path = "/api/libraries",
    request_body = CreateLibraryRequest,
    responses(
        (status = 201, description = "Library created", body = LibraryResponse)
    )
)]
pub async fn create_library(
    State(ctx): State<AppContext>,
    Json(payload): Json<CreateLibraryRequest>,
) -> Result<impl IntoResponse, AppError> {
    if payload.name.is_empty() {
        return Err(sf_core::Error::Validation("name is required".into()).into());
    }

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let config = if payload.config.is_null() {
        serde_json::json!({})
    } else {
        payload.config
    };
    let lib = sf_db::queries::libraries::create_library(
        &conn,
        &payload.name,
        &payload.media_type,
        &payload.paths,
        &config,
    )?;

    ctx.event_bus.broadcast(
        sf_core::events::EventCategory::User,
        sf_core::events::EventPayload::LibraryCreated {
            library_id: lib.id,
            name: lib.name.clone(),
        },
    );

    Ok((StatusCode::CREATED, Json(LibraryResponse::from_model(&lib))))
}

/// GET /api/libraries/:id
#[utoipa::path(
    get,
    path = "/api/libraries/{id}",
    params(("id" = String, Path, description = "Library ID")),
    responses(
        (status = 200, description = "Library details", body = LibraryResponse),
        (status = 404, description = "Library not found")
    )
)]
pub async fn get_library(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> Result<Json<LibraryResponse>, AppError> {
    let lib_id: sf_core::LibraryId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid library ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let lib = sf_db::queries::libraries::get_library(&conn, lib_id)?
        .ok_or_else(|| sf_core::Error::not_found("library", lib_id))?;

    Ok(Json(LibraryResponse::from_model(&lib)))
}

/// DELETE /api/libraries/:id
#[utoipa::path(
    delete,
    path = "/api/libraries/{id}",
    params(("id" = String, Path, description = "Library ID")),
    responses(
        (status = 204, description = "Library deleted"),
        (status = 404, description = "Library not found")
    )
)]
pub async fn delete_library(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let lib_id: sf_core::LibraryId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid library ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let deleted = sf_db::queries::libraries::delete_library(&conn, lib_id)?;

    if !deleted {
        return Err(sf_core::Error::not_found("library", lib_id).into());
    }

    ctx.event_bus.broadcast(
        sf_core::events::EventCategory::User,
        sf_core::events::EventPayload::LibraryDeleted {
            library_id: lib_id,
        },
    );

    Ok(StatusCode::NO_CONTENT)
}

/// Query parameters for library item listing.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct LibraryItemsParams {
    #[serde(default)]
    pub offset: i64,
    #[serde(default = "default_items_limit")]
    pub limit: i64,
}

fn default_items_limit() -> i64 {
    50
}

/// GET /api/libraries/:id/items
#[utoipa::path(
    get,
    path = "/api/libraries/{id}/items",
    params(
        ("id" = String, Path, description = "Library ID"),
        LibraryItemsParams,
    ),
    responses(
        (status = 200, description = "Items in library", body = Vec<ItemResponse>),
        (status = 404, description = "Library not found")
    )
)]
pub async fn list_library_items(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
    Query(params): Query<LibraryItemsParams>,
) -> Result<Json<Vec<ItemResponse>>, AppError> {
    let lib_id: sf_core::LibraryId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid library ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;

    // Verify library exists.
    sf_db::queries::libraries::get_library(&conn, lib_id)?
        .ok_or_else(|| sf_core::Error::not_found("library", lib_id))?;

    let items =
        sf_db::queries::items::list_items_by_library(&conn, lib_id, params.offset, params.limit)?;
    let responses: Vec<ItemResponse> = items.iter().map(ItemResponse::from_model).collect();
    Ok(Json(responses))
}

/// GET /api/libraries/:id/recent
#[utoipa::path(
    get,
    path = "/api/libraries/{id}/recent",
    params(("id" = String, Path, description = "Library ID")),
    responses(
        (status = 200, description = "Recently added items (last 7 days)", body = Vec<ItemResponse>),
        (status = 404, description = "Library not found")
    )
)]
pub async fn list_library_recent(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> Result<Json<Vec<ItemResponse>>, AppError> {
    let lib_id: sf_core::LibraryId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid library ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;

    // Verify library exists.
    sf_db::queries::libraries::get_library(&conn, lib_id)?
        .ok_or_else(|| sf_core::Error::not_found("library", lib_id))?;

    let items = sf_db::queries::items::list_recent_items_by_library(&conn, lib_id, 7)?;
    let responses: Vec<ItemResponse> = items.iter().map(ItemResponse::from_model).collect();
    Ok(Json(responses))
}

/// POST /api/libraries/:id/scan
#[utoipa::path(
    post,
    path = "/api/libraries/{id}/scan",
    params(("id" = String, Path, description = "Library ID")),
    responses(
        (status = 202, description = "Scan queued"),
        (status = 404, description = "Library not found")
    )
)]
pub async fn scan_library(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let lib_id: sf_core::LibraryId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid library ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let lib = sf_db::queries::libraries::get_library(&conn, lib_id)?
        .ok_or_else(|| sf_core::Error::not_found("library", lib_id))?;

    ctx.event_bus.broadcast(
        sf_core::events::EventCategory::User,
        sf_core::events::EventPayload::LibraryScanStarted {
            library_id: lib_id,
        },
    );

    // Spawn the scan in a background task.
    let scan_ctx = ctx.clone();
    tokio::spawn(async move {
        crate::scanner::scan_library(scan_ctx, lib).await;
    });

    Ok((StatusCode::ACCEPTED, Json(serde_json::json!({"status": "scan_queued"}))))
}
