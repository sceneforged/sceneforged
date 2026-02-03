//! Image serving route handlers.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::context::AppContext;
use crate::error::AppError;

/// GET /api/images/:item_id/:type/:size
pub async fn get_image(
    State(ctx): State<AppContext>,
    Path((item_id, image_type, _size)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let item_id: sf_core::ItemId = item_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let images = sf_db::queries::images::list_images_by_item(&conn, item_id)?;

    let image = images
        .iter()
        .find(|img| img.image_type == image_type)
        .ok_or_else(|| sf_core::Error::not_found("image", format!("{item_id}/{image_type}")))?;

    // Read the image from disk.
    let data = std::fs::read(&image.path).map_err(|e| {
        sf_core::Error::Internal(format!("Failed to read image {}: {e}", image.path))
    })?;

    let content_type = if image.path.ends_with(".png") {
        "image/png"
    } else if image.path.ends_with(".webp") {
        "image/webp"
    } else {
        "image/jpeg"
    };

    Ok((StatusCode::OK, [(axum::http::header::CONTENT_TYPE, content_type)], data))
}
