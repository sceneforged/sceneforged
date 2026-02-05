//! Playback and favorites route handlers.

use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use sf_core::UserId;

use crate::context::AppContext;
use crate::error::AppError;
use crate::routes::items::ItemResponse;

// ---------------------------------------------------------------------------
// Request / response schemas
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct PlaybackListParams {
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateProgressRequest {
    pub position_secs: f64,
    #[serde(default)]
    pub completed: bool,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PlaybackResponse {
    pub item_id: String,
    pub position_secs: f64,
    pub completed: bool,
    pub play_count: i32,
    pub last_played_at: String,
}

impl PlaybackResponse {
    fn from_model(pb: &sf_db::models::Playback) -> Self {
        Self {
            item_id: pb.item_id.to_string(),
            position_secs: pb.position_secs,
            completed: pb.completed,
            play_count: pb.play_count,
            last_played_at: pb.last_played_at.clone(),
        }
    }
}

/// Enriched continue-watching entry with full item data.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ContinueWatchingEntry {
    pub item: ItemResponse,
    pub position_secs: f64,
    pub completed: bool,
    pub play_count: i32,
    pub last_played_at: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct FavoriteResponse {
    pub item_id: String,
    pub created_at: String,
}

impl FavoriteResponse {
    fn from_model(fav: &sf_db::models::Favorite) -> Self {
        Self {
            item_id: fav.item_id.to_string(),
            created_at: fav.created_at.clone(),
        }
    }
}

/// Enriched favorite entry with full item data.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct FavoriteEntry {
    pub item: ItemResponse,
    pub created_at: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct UserDataResponse {
    pub playback: Option<PlaybackResponse>,
    pub is_favorite: bool,
}

// ---------------------------------------------------------------------------
// Playback routes
// ---------------------------------------------------------------------------

/// GET /api/playback/continue
///
/// List in-progress items (position > 0, not completed) for "Continue Watching".
/// Returns enriched entries with full item data to avoid N+1 queries.
#[utoipa::path(
    get,
    path = "/api/playback/continue",
    params(PlaybackListParams),
    responses(
        (status = 200, description = "In-progress items with item data", body = Vec<ContinueWatchingEntry>)
    )
)]
pub async fn continue_watching(
    State(ctx): State<AppContext>,
    Extension(user_id): Extension<UserId>,
    Query(params): Query<PlaybackListParams>,
) -> Result<Json<Vec<ContinueWatchingEntry>>, AppError> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let playbacks = sf_db::queries::playback::list_in_progress(&conn, user_id, params.limit)?;

    let mut entries = Vec::with_capacity(playbacks.len());
    for pb in &playbacks {
        if let Some(item) = sf_db::queries::items::get_item(&conn, pb.item_id)? {
            entries.push(ContinueWatchingEntry {
                item: ItemResponse::from_model(&item),
                position_secs: pb.position_secs,
                completed: pb.completed,
                play_count: pb.play_count,
                last_played_at: pb.last_played_at.clone(),
            });
        }
    }

    Ok(Json(entries))
}

/// GET /api/playback/:item_id
///
/// Get playback state for a specific item.
#[utoipa::path(
    get,
    path = "/api/playback/{item_id}",
    params(("item_id" = String, Path, description = "Item ID")),
    responses(
        (status = 200, description = "Playback state", body = PlaybackResponse),
        (status = 404, description = "No playback state")
    )
)]
pub async fn get_playback(
    State(ctx): State<AppContext>,
    Extension(user_id): Extension<UserId>,
    Path(item_id): Path<String>,
) -> Result<Json<PlaybackResponse>, AppError> {
    let item_id: sf_core::ItemId = item_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let pb = sf_db::queries::playback::get_playback(&conn, user_id, item_id)?
        .ok_or_else(|| sf_core::Error::not_found("playback", item_id))?;
    Ok(Json(PlaybackResponse::from_model(&pb)))
}

/// POST /api/playback/:item_id/progress
///
/// Update playback position for an item.
#[utoipa::path(
    post,
    path = "/api/playback/{item_id}/progress",
    params(("item_id" = String, Path, description = "Item ID")),
    request_body = UpdateProgressRequest,
    responses(
        (status = 200, description = "Updated playback state", body = PlaybackResponse)
    )
)]
pub async fn update_progress(
    State(ctx): State<AppContext>,
    Extension(user_id): Extension<UserId>,
    Path(item_id): Path<String>,
    Json(body): Json<UpdateProgressRequest>,
) -> Result<Json<PlaybackResponse>, AppError> {
    let item_id: sf_core::ItemId = item_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let pb = sf_db::queries::playback::upsert_playback(
        &conn,
        user_id,
        item_id,
        body.position_secs,
        body.completed,
    )?;
    Ok(Json(PlaybackResponse::from_model(&pb)))
}

/// POST /api/playback/:item_id/played
///
/// Mark an item as played (completed).
#[utoipa::path(
    post,
    path = "/api/playback/{item_id}/played",
    params(("item_id" = String, Path, description = "Item ID")),
    responses(
        (status = 200, description = "Marked as played", body = PlaybackResponse)
    )
)]
pub async fn mark_played(
    State(ctx): State<AppContext>,
    Extension(user_id): Extension<UserId>,
    Path(item_id): Path<String>,
) -> Result<Json<PlaybackResponse>, AppError> {
    let item_id: sf_core::ItemId = item_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let pb = sf_db::queries::playback::mark_played(&conn, user_id, item_id)?;
    Ok(Json(PlaybackResponse::from_model(&pb)))
}

/// POST /api/playback/:item_id/unplayed
///
/// Mark an item as unplayed (reset position).
#[utoipa::path(
    post,
    path = "/api/playback/{item_id}/unplayed",
    params(("item_id" = String, Path, description = "Item ID")),
    responses(
        (status = 200, description = "Marked as unplayed")
    )
)]
pub async fn mark_unplayed(
    State(ctx): State<AppContext>,
    Extension(user_id): Extension<UserId>,
    Path(item_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let item_id: sf_core::ItemId = item_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    sf_db::queries::playback::mark_unplayed(&conn, user_id, item_id)?;
    Ok(StatusCode::OK)
}

// ---------------------------------------------------------------------------
// Favorites routes
// ---------------------------------------------------------------------------

/// GET /api/favorites
///
/// List the user's favorite items. Returns enriched entries with full item data.
#[utoipa::path(
    get,
    path = "/api/favorites",
    params(PlaybackListParams),
    responses(
        (status = 200, description = "Favorite items with item data", body = Vec<FavoriteEntry>)
    )
)]
pub async fn list_favorites(
    State(ctx): State<AppContext>,
    Extension(user_id): Extension<UserId>,
    Query(params): Query<PlaybackListParams>,
) -> Result<Json<Vec<FavoriteEntry>>, AppError> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let favs = sf_db::queries::favorites::list_favorites(&conn, user_id, params.limit)?;

    let mut entries = Vec::with_capacity(favs.len());
    for fav in &favs {
        if let Some(item) = sf_db::queries::items::get_item(&conn, fav.item_id)? {
            entries.push(FavoriteEntry {
                item: ItemResponse::from_model(&item),
                created_at: fav.created_at.clone(),
            });
        }
    }

    Ok(Json(entries))
}

/// POST /api/favorites/:item_id
///
/// Add an item to favorites.
#[utoipa::path(
    post,
    path = "/api/favorites/{item_id}",
    params(("item_id" = String, Path, description = "Item ID")),
    responses(
        (status = 201, description = "Added to favorites", body = FavoriteResponse)
    )
)]
pub async fn add_favorite(
    State(ctx): State<AppContext>,
    Extension(user_id): Extension<UserId>,
    Path(item_id): Path<String>,
) -> Result<(StatusCode, Json<FavoriteResponse>), AppError> {
    let item_id: sf_core::ItemId = item_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let fav = sf_db::queries::favorites::add_favorite(&conn, user_id, item_id)?;
    Ok((StatusCode::CREATED, Json(FavoriteResponse::from_model(&fav))))
}

/// DELETE /api/favorites/:item_id
///
/// Remove an item from favorites.
#[utoipa::path(
    delete,
    path = "/api/favorites/{item_id}",
    params(("item_id" = String, Path, description = "Item ID")),
    responses(
        (status = 200, description = "Removed from favorites")
    )
)]
pub async fn remove_favorite(
    State(ctx): State<AppContext>,
    Extension(user_id): Extension<UserId>,
    Path(item_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let item_id: sf_core::ItemId = item_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    sf_db::queries::favorites::remove_favorite(&conn, user_id, item_id)?;
    Ok(StatusCode::OK)
}

/// GET /api/playback/:item_id/user-data
///
/// Get combined playback + favorite state for an item.
#[utoipa::path(
    get,
    path = "/api/playback/{item_id}/user-data",
    params(("item_id" = String, Path, description = "Item ID")),
    responses(
        (status = 200, description = "User data for item", body = UserDataResponse)
    )
)]
pub async fn get_user_data(
    State(ctx): State<AppContext>,
    Extension(user_id): Extension<UserId>,
    Path(item_id): Path<String>,
) -> Result<Json<UserDataResponse>, AppError> {
    let item_id: sf_core::ItemId = item_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let playback = sf_db::queries::playback::get_playback(&conn, user_id, item_id)?
        .map(|pb| PlaybackResponse::from_model(&pb));
    let is_favorite = sf_db::queries::favorites::get_favorite(&conn, user_id, item_id)?
        .is_some();

    Ok(Json(UserDataResponse {
        playback,
        is_favorite,
    }))
}
