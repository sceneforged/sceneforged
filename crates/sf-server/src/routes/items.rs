//! Item query route handlers.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
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
    pub hls_ready: bool,
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
            hls_ready: mf.hls_ready,
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

/// Paginated items response.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PaginatedItems {
    pub items: Vec<ItemResponse>,
    pub total: i64,
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
    pub scan_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scan_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_files: Option<Vec<MediaFileResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<ImageResponse>>,
}

impl ItemResponse {
    pub(crate) fn from_model(item: &sf_db::models::Item) -> Self {
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
            scan_status: item.scan_status.clone(),
            scan_error: item.scan_error.clone(),
            source_file_path: item.source_file_path.clone(),
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

/// GET /api/items/:id/files
#[utoipa::path(
    get,
    path = "/api/items/{id}/files",
    params(("id" = String, Path, description = "Item ID")),
    responses(
        (status = 200, description = "Media files for item", body = Vec<MediaFileResponse>),
        (status = 404, description = "Item not found")
    )
)]
pub async fn list_item_files(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> Result<Json<Vec<MediaFileResponse>>, AppError> {
    let item_id: sf_core::ItemId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;

    // Verify item exists.
    sf_db::queries::items::get_item(&conn, item_id)?
        .ok_or_else(|| sf_core::Error::not_found("item", item_id))?;

    let media_files = sf_db::queries::media_files::list_media_files_by_item(&conn, item_id)?;
    let responses: Vec<MediaFileResponse> =
        media_files.iter().map(MediaFileResponse::from_model).collect();
    Ok(Json(responses))
}

/// Query parameters for search.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct SearchParams {
    pub q: String,
    #[serde(default = "default_search_limit")]
    pub limit: i64,
    pub library_id: Option<String>,
    pub item_kind: Option<String>,
}

fn default_search_limit() -> i64 {
    20
}

/// GET /api/search
#[utoipa::path(
    get,
    path = "/api/search",
    params(SearchParams),
    responses(
        (status = 200, description = "Search results", body = Vec<ItemResponse>)
    )
)]
pub async fn search_items(
    State(ctx): State<AppContext>,
    Query(params): Query<SearchParams>,
) -> Result<Json<Vec<ItemResponse>>, AppError> {
    if params.q.is_empty() {
        return Ok(Json(Vec::new()));
    }

    let conn = sf_db::pool::get_conn(&ctx.db)?;

    let library_id = params
        .library_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<sf_core::LibraryId>())
        .transpose()
        .map_err(|_| sf_core::Error::Validation("Invalid library_id".into()))?;

    let items = sf_db::queries::items::search_items_fts(
        &conn,
        &params.q,
        library_id,
        params.item_kind.as_deref().filter(|s| !s.is_empty()),
        params.limit,
    )
    .or_else(|_| sf_db::queries::items::search_items(&conn, &params.q, params.limit))?;

    let responses: Vec<ItemResponse> = items.iter().map(ItemResponse::from_model).collect();
    Ok(Json(responses))
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

/// POST /api/items/:id/retry-probe
///
/// Re-probes an item that has scan_status='error'. On success, creates the
/// media_file and clears the error status. On failure, updates the error message.
pub async fn retry_probe(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let item_id: sf_core::ItemId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let item = sf_db::queries::items::get_item(&conn, item_id)?
        .ok_or_else(|| sf_core::Error::not_found("item", item_id))?;

    if item.scan_status.as_deref() != Some("error") {
        return Err(sf_core::Error::Validation(
            "Item is not in error state".into(),
        )
        .into());
    }

    let source_path = item.source_file_path.as_deref().ok_or_else(|| {
        sf_core::Error::Validation("Item has no source_file_path for retry".into())
    })?;

    let path = std::path::PathBuf::from(source_path);
    if !path.exists() {
        return Err(
            sf_core::Error::Validation("Source file no longer exists on disk".into()).into(),
        );
    }

    // Probe the file.
    let prober = ctx.prober.clone();
    let probe_path = path.clone();
    let media_info = tokio::task::spawn_blocking(move || prober.probe(&probe_path))
        .await
        .map_err(|e| sf_core::Error::Internal(format!("Probe task join error: {e}")))?;

    match media_info {
        Ok(info) => {
            let file_size = std::fs::metadata(&path)
                .map(|m| m.len() as i64)
                .unwrap_or(0);
            let file_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let file_path_str = path.to_string_lossy().to_string();

            let profile = info.classify_profile();
            let role = if profile == sf_core::Profile::B {
                "universal"
            } else {
                "source"
            };
            let video = info.primary_video();
            let audio = info.primary_audio();
            let container = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase());
            let video_codec = video.map(|v| format!("{}", v.codec));
            let audio_codec = audio.map(|a| format!("{}", a.codec));
            let resolution_width = video.map(|v| v.width as i32);
            let resolution_height = video.map(|v| v.height as i32);
            let hdr_format = video.and_then(|v| {
                if v.hdr_format == sf_core::HdrFormat::Sdr {
                    None
                } else {
                    Some(format!("{}", v.hdr_format))
                }
            });
            let has_dv = video.map_or(false, |v| v.dolby_vision.is_some());
            let dv_profile = video
                .and_then(|v| v.dolby_vision.as_ref())
                .map(|dv| dv.profile as i32);
            let duration_secs = info.duration.map(|d| d.as_secs_f64());

            // Create media_file.
            let mf = sf_db::queries::media_files::create_media_file(
                &conn,
                item_id,
                &file_path_str,
                &file_name,
                file_size,
                container.as_deref(),
                video_codec.as_deref(),
                audio_codec.as_deref(),
                resolution_width,
                resolution_height,
                hdr_format.as_deref(),
                has_dv,
                dv_profile,
                role,
                &profile.to_string(),
                duration_secs,
            )?;

            // Store subtitle tracks.
            for (idx, sub) in info.subtitle_tracks.iter().enumerate() {
                let _ = sf_db::queries::subtitle_tracks::create_subtitle_track(
                    &conn,
                    mf.id,
                    idx as i32,
                    &sub.codec,
                    sub.language.as_deref(),
                    sub.forced,
                    sub.default,
                );
            }

            // Clear error status.
            sf_db::queries::items::update_item_scan_status(&conn, item_id, None, None)?;

            ctx.event_bus.broadcast(
                sf_core::events::EventCategory::User,
                sf_core::events::EventPayload::ItemStatusChanged {
                    item_id,
                    library_id: item.library_id,
                    scan_status: "ready".into(),
                },
            );

            Ok((StatusCode::OK, Json(serde_json::json!({"status": "ok"}))))
        }
        Err(e) => {
            let error_msg = format!("{e}");
            sf_db::queries::items::update_item_scan_status(
                &conn,
                item_id,
                Some("error"),
                Some(&error_msg),
            )?;

            Ok((
                StatusCode::OK,
                Json(serde_json::json!({"status": "error", "error": error_msg})),
            ))
        }
    }
}
