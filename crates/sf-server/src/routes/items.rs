//! Item query route handlers.

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::context::AppContext;
use crate::error::AppError;

/// Query parameters for listing items.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
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

/// Item response.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ItemResponse {
    pub id: String,
    pub library_id: String,
    pub item_kind: String,
    pub name: String,
    pub sort_name: Option<String>,
    pub year: Option<i32>,
    pub overview: Option<String>,
    pub runtime_minutes: Option<i32>,
    pub community_rating: Option<f64>,
    pub provider_ids: String,
    pub parent_id: Option<String>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
}

impl ItemResponse {
    fn from_model(item: &sf_db::models::Item) -> Self {
        Self {
            id: item.id.to_string(),
            library_id: item.library_id.to_string(),
            item_kind: item.item_kind.clone(),
            name: item.name.clone(),
            sort_name: item.sort_name.clone(),
            year: item.year,
            overview: item.overview.clone(),
            runtime_minutes: item.runtime_minutes,
            community_rating: item.community_rating,
            provider_ids: item.provider_ids.clone(),
            parent_id: item.parent_id.map(|id| id.to_string()),
            season_number: item.season_number,
            episode_number: item.episode_number,
            created_at: item.created_at.clone(),
            updated_at: item.updated_at.clone(),
        }
    }
}

/// GET /api/items
#[utoipa::path(
    get,
    path = "/api/items",
    params(ListItemsParams),
    responses(
        (status = 200, description = "List items", body = Vec<ItemResponse>)
    )
)]
pub async fn list_items(
    State(ctx): State<AppContext>,
    Query(params): Query<ListItemsParams>,
) -> Result<Json<Vec<ItemResponse>>, AppError> {
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

    let responses: Vec<ItemResponse> = items.iter().map(ItemResponse::from_model).collect();
    Ok(Json(responses))
}

/// GET /api/items/:id
#[utoipa::path(
    get,
    path = "/api/items/{id}",
    params(("id" = String, Path, description = "Item ID")),
    responses(
        (status = 200, description = "Item details", body = ItemResponse),
        (status = 404, description = "Item not found")
    )
)]
pub async fn get_item(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> Result<Json<ItemResponse>, AppError> {
    let item_id: sf_core::ItemId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let item = sf_db::queries::items::get_item(&conn, item_id)?
        .ok_or_else(|| sf_core::Error::not_found("item", item_id))?;

    Ok(Json(ItemResponse::from_model(&item)))
}
