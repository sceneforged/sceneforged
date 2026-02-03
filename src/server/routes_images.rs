//! Image serving and metadata refresh API routes.
//!
//! Provides endpoints for serving stored images at various sizes,
//! listing images for items, and triggering metadata re-enrichment.

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use sceneforged_common::{ImageId, ItemId};
use sceneforged_db::queries::{images, items, libraries};
use serde::Deserialize;
use tokio_util::io::ReaderStream;

use super::AppContext;
use crate::metadata::queue::EnrichmentJob;

/// Create image-related routes.
pub fn image_routes() -> Router<AppContext> {
    Router::new()
        .route("/images/:image_id", get(serve_image))
        .route("/items/:item_id/images", get(list_item_images))
        .route("/items/:item_id/images/primary", get(get_primary_image))
        .route(
            "/items/:item_id/refresh-metadata",
            post(refresh_metadata),
        )
}

// ============================================================================
// Request types
// ============================================================================

/// Query parameters for the image serving endpoint.
#[derive(Debug, Deserialize)]
pub struct ImageSizeQuery {
    /// Desired image size variant (small, medium, large, original).
    /// Defaults to medium if not specified.
    #[serde(default = "default_size")]
    pub size: String,
}

fn default_size() -> String {
    "medium".to_string()
}

// ============================================================================
// Handlers
// ============================================================================

/// Serve an image file by ID.
///
/// Returns the image bytes with appropriate caching headers.
/// Supports size variants via the `size` query parameter.
async fn serve_image(
    State(ctx): State<AppContext>,
    Path(image_id): Path<String>,
    Query(query): Query<ImageSizeQuery>,
) -> impl IntoResponse {
    let Some(ref image_service) = ctx.image_service else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Image service not available"})),
        )
            .into_response();
    };

    let id = match image_id.parse::<uuid::Uuid>() {
        Ok(uuid) => ImageId::from(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid image ID"})),
            )
                .into_response()
        }
    };

    let size = match parse_image_size(&query.size) {
        Some(s) => s,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "Invalid size. Valid values: small, medium, large, original"
                })),
            )
                .into_response()
        }
    };

    let path = match image_service.get_image_path(id, size) {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Image not found"})),
            )
                .into_response()
        }
    };

    // Open the file and stream it back
    let file = match tokio::fs::File::open(&path).await {
        Ok(f) => f,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Image file not found on disk"})),
            )
                .into_response()
        }
    };

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    (
        StatusCode::OK,
        [
            (
                header::CACHE_CONTROL,
                "public, max-age=31536000, immutable",
            ),
            (header::CONTENT_TYPE, "image/jpeg"),
        ],
        body,
    )
        .into_response()
}

/// List all images for an item.
///
/// Returns a JSON array of image records. Returns an empty array
/// if the item has no images.
async fn list_item_images(
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

    match images::get_images_for_item(&conn, id) {
        Ok(image_list) => Json(image_list).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Get the primary image for an item.
///
/// Returns the primary image record, or 404 if no primary image exists.
async fn get_primary_image(
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

    match images::get_primary_image(&conn, id) {
        Ok(Some(image)) => Json(image).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "No primary image found for this item"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Manually trigger a metadata refresh for an item.
///
/// Looks up the item to get its title, year, and media type, then submits
/// an enrichment job to the background queue. Returns 202 Accepted immediately.
async fn refresh_metadata(
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

    let Some(ref enrichment_queue) = ctx.enrichment_queue else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Metadata enrichment not configured"})),
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

    // Look up the item to get title, year, and media type
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

    // Look up the library to get the media type
    let library = match libraries::get_library(&conn, item.library_id) {
        Ok(Some(lib)) => lib,
        Ok(None) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Item's library not found"})),
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

    let year = item.production_year.and_then(|y| u16::try_from(y).ok());

    let job = EnrichmentJob {
        item_id: id,
        title: item.name.clone(),
        year,
        media_type: library.media_type,
    };

    match enrichment_queue.submit(job).await {
        Ok(()) => (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({
                "status": "queued",
                "message": format!("Metadata refresh queued for '{}'", item.name)
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to queue enrichment: {}", e)})),
        )
            .into_response(),
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Parse a size string into an `ImageSize` enum value.
fn parse_image_size(s: &str) -> Option<crate::metadata::images::ImageSize> {
    use crate::metadata::images::ImageSize;
    match s.to_lowercase().as_str() {
        "small" => Some(ImageSize::Small),
        "medium" => Some(ImageSize::Medium),
        "large" => Some(ImageSize::Large),
        "original" => Some(ImageSize::Original),
        _ => None,
    }
}
