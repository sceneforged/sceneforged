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

/// Media file response (subset of fields for API).
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MediaFileResponse {
    pub id: String,
    pub file_path: String,
    pub file_name: String,
    pub file_size: i64,
    pub container: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub resolution_width: Option<i32>,
    pub resolution_height: Option<i32>,
    pub role: String,
    pub profile: String,
    pub duration_secs: Option<f64>,
}

impl MediaFileResponse {
    fn from_model(mf: &sf_db::models::MediaFile) -> Self {
        Self {
            id: mf.id.to_string(),
            file_path: mf.file_path.clone(),
            file_name: mf.file_name.clone(),
            file_size: mf.file_size,
            container: mf.container.clone(),
            video_codec: mf.video_codec.clone(),
            audio_codec: mf.audio_codec.clone(),
            resolution_width: mf.resolution_width,
            resolution_height: mf.resolution_height,
            role: mf.role.clone(),
            profile: mf.profile.clone(),
            duration_secs: mf.duration_secs,
        }
    }
}

/// Image response.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ImageResponse {
    pub id: String,
    pub image_type: String,
    pub path: String,
    pub provider: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

impl ImageResponse {
    fn from_model(img: &sf_db::models::Image) -> Self {
        Self {
            id: img.id.to_string(),
            image_type: img.image_type.clone(),
            path: img.path.clone(),
            provider: img.provider.clone(),
            width: img.width,
            height: img.height,
        }
    }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_files: Option<Vec<MediaFileResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<ImageResponse>>,
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
            media_files: None,
            images: None,
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

    let media_files = sf_db::queries::media_files::list_media_files_by_item(&conn, item_id)?;
    let images = sf_db::queries::images::list_images_by_item(&conn, item_id)?;

    let mut resp = ItemResponse::from_model(&item);
    resp.media_files = Some(media_files.iter().map(MediaFileResponse::from_model).collect());
    resp.images = Some(images.iter().map(ImageResponse::from_model).collect());

    Ok(Json(resp))
}

/// GET /api/items/:id/children
#[utoipa::path(
    get,
    path = "/api/items/{id}/children",
    params(("id" = String, Path, description = "Parent item ID")),
    responses(
        (status = 200, description = "Child items", body = Vec<ItemResponse>)
    )
)]
pub async fn list_children(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> Result<Json<Vec<ItemResponse>>, AppError> {
    let parent_id: sf_core::ItemId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let children = sf_db::queries::items::list_children(&conn, parent_id)?;
    let responses: Vec<ItemResponse> = children.iter().map(ItemResponse::from_model).collect();
    Ok(Json(responses))
}
