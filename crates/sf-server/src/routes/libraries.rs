//! Library CRUD route handlers.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;

use crate::context::AppContext;
use crate::error::AppError;

/// Request body for creating a library.
#[derive(Debug, Deserialize)]
pub struct CreateLibraryRequest {
    pub name: String,
    pub media_type: String,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub config: serde_json::Value,
}

/// Convert a db Library model to a JSON value.
fn library_to_json(lib: &sf_db::models::Library) -> serde_json::Value {
    serde_json::json!({
        "id": lib.id.to_string(),
        "name": lib.name,
        "media_type": lib.media_type,
        "paths": lib.paths,
        "config": lib.config,
        "created_at": lib.created_at,
    })
}

/// GET /api/libraries
pub async fn list_libraries(
    State(ctx): State<AppContext>,
) -> Result<impl IntoResponse, AppError> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let libs = sf_db::queries::libraries::list_libraries(&conn)?;
    let json: Vec<serde_json::Value> = libs.iter().map(library_to_json).collect();
    Ok(Json(json))
}

/// POST /api/libraries
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

    Ok((StatusCode::CREATED, Json(library_to_json(&lib))))
}

/// GET /api/libraries/:id
pub async fn get_library(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let lib_id: sf_core::LibraryId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid library ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let lib = sf_db::queries::libraries::get_library(&conn, lib_id)?
        .ok_or_else(|| sf_core::Error::not_found("library", lib_id))?;

    Ok(Json(library_to_json(&lib)))
}

/// DELETE /api/libraries/:id
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

/// POST /api/libraries/:id/scan
pub async fn scan_library(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let lib_id: sf_core::LibraryId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid library ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let _lib = sf_db::queries::libraries::get_library(&conn, lib_id)?
        .ok_or_else(|| sf_core::Error::not_found("library", lib_id))?;

    ctx.event_bus.broadcast(
        sf_core::events::EventCategory::User,
        sf_core::events::EventPayload::LibraryScanStarted {
            library_id: lib_id,
        },
    );

    // The actual scan runs asynchronously via the file watcher / processor.
    Ok((StatusCode::ACCEPTED, Json(serde_json::json!({"status": "scan_queued"}))))
}
