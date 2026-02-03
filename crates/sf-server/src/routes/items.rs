//! Item query route handlers.

use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;

use crate::context::AppContext;
use crate::error::AppError;

/// Query parameters for listing items.
#[derive(Debug, Deserialize)]
pub struct ListItemsParams {
    pub library_id: Option<String>,
    pub search: Option<String>,
    #[serde(default = "default_offset")]
    pub offset: i64,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_offset() -> i64 {
    0
}

fn default_limit() -> i64 {
    50
}

/// Convert a db Item model to a JSON value.
fn item_to_json(item: &sf_db::models::Item) -> serde_json::Value {
    serde_json::json!({
        "id": item.id.to_string(),
        "library_id": item.library_id.to_string(),
        "item_kind": item.item_kind,
        "name": item.name,
        "sort_name": item.sort_name,
        "year": item.year,
        "overview": item.overview,
        "runtime_minutes": item.runtime_minutes,
        "community_rating": item.community_rating,
        "provider_ids": item.provider_ids,
        "parent_id": item.parent_id.map(|id| id.to_string()),
        "season_number": item.season_number,
        "episode_number": item.episode_number,
        "created_at": item.created_at,
        "updated_at": item.updated_at,
    })
}

/// GET /api/items
pub async fn list_items(
    State(ctx): State<AppContext>,
    Query(params): Query<ListItemsParams>,
) -> Result<impl IntoResponse, AppError> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;

    let items = if let Some(ref query) = params.search {
        sf_db::queries::items::search_items(&conn, query, params.limit)?
    } else if let Some(ref lib_id_str) = params.library_id {
        let lib_id: sf_core::LibraryId = lib_id_str
            .parse()
            .map_err(|_| sf_core::Error::Validation("Invalid library_id".into()))?;
        sf_db::queries::items::list_items_by_library(&conn, lib_id, params.offset, params.limit)?
    } else {
        // Without a library_id or search, return an empty list.
        Vec::new()
    };

    let json: Vec<serde_json::Value> = items.iter().map(item_to_json).collect();
    Ok(Json(json))
}

/// GET /api/items/:id
pub async fn get_item(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let item_id: sf_core::ItemId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let item = sf_db::queries::items::get_item(&conn, item_id)?
        .ok_or_else(|| sf_core::Error::not_found("item", item_id))?;

    Ok(Json(item_to_json(&item)))
}
