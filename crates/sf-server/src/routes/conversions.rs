//! Conversion job API route handlers.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::context::AppContext;
use crate::error::AppError;

/// Query parameters for listing conversion jobs.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ListConversionsParams {
    pub status: Option<String>,
    #[serde(default)]
    pub offset: i64,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    50
}

/// Request body for submitting a conversion.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SubmitConversionRequest {
    pub item_id: String,
    pub media_file_id: Option<String>,
}

/// Conversion job response.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ConversionJobResponse {
    pub id: String,
    pub item_id: String,
    pub item_name: Option<String>,
    pub media_file_id: Option<String>,
    pub source_media_file_id: Option<String>,
    pub status: String,
    pub progress_pct: f64,
    pub encode_fps: Option<f64>,
    pub eta_secs: Option<i64>,
    pub error: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

impl ConversionJobResponse {
    fn from_model(job: &sf_db::models::ConversionJob, item_name: Option<String>) -> Self {
        Self {
            id: job.id.to_string(),
            item_id: job.item_id.to_string(),
            item_name,
            media_file_id: job.media_file_id.map(|id| id.to_string()),
            source_media_file_id: job.source_media_file_id.map(|id| id.to_string()),
            status: job.status.clone(),
            progress_pct: job.progress_pct,
            encode_fps: job.encode_fps,
            eta_secs: job.eta_secs,
            error: job.error.clone(),
            created_at: job.created_at.clone(),
            started_at: job.started_at.clone(),
            completed_at: job.completed_at.clone(),
        }
    }
}

/// POST /api/conversions/submit
#[utoipa::path(
    post,
    path = "/api/conversions/submit",
    request_body = SubmitConversionRequest,
    responses(
        (status = 201, description = "Conversion submitted", body = ConversionJobResponse)
    )
)]
pub async fn submit_conversion(
    State(ctx): State<AppContext>,
    Json(payload): Json<SubmitConversionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let item_id: sf_core::ItemId = payload
        .item_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item_id".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;

    // Verify item exists.
    sf_db::queries::items::get_item(&conn, item_id)?
        .ok_or_else(|| sf_core::Error::not_found("item", item_id))?;

    // Check for existing active conversion.
    if sf_db::queries::conversion_jobs::has_active_conversion_for_item(&conn, item_id)? {
        return Err(sf_core::Error::Conflict(
            "Item already has an active conversion job".into(),
        )
        .into());
    }

    // Resolve source media file — use provided ID or pick the first source file.
    let source_mf_id = if let Some(ref mf_id_str) = payload.media_file_id {
        mf_id_str
            .parse::<sf_core::MediaFileId>()
            .map_err(|_| sf_core::Error::Validation("Invalid media_file_id".into()))?
    } else {
        let files = sf_db::queries::media_files::list_media_files_by_item(&conn, item_id)?;
        let source = files
            .iter()
            .find(|f| f.role == "source")
            .or(files.first())
            .ok_or_else(|| {
                sf_core::Error::Validation("No media files found for item".into())
            })?;
        source.id
    };

    let job = sf_db::queries::conversion_jobs::create_conversion_job(&conn, item_id, source_mf_id)?;

    let item_name = sf_db::queries::items::get_item(&conn, item_id)?
        .map(|i| i.name);

    ctx.event_bus.broadcast(
        sf_core::events::EventCategory::Admin,
        sf_core::events::EventPayload::ConversionQueued { job_id: job.id },
    );

    Ok((
        StatusCode::CREATED,
        Json(ConversionJobResponse::from_model(&job, item_name)),
    ))
}

/// GET /api/conversions
#[utoipa::path(
    get,
    path = "/api/conversions",
    params(ListConversionsParams),
    responses(
        (status = 200, description = "List conversion jobs", body = Vec<ConversionJobResponse>)
    )
)]
pub async fn list_conversions(
    State(ctx): State<AppContext>,
    Query(params): Query<ListConversionsParams>,
) -> Result<Json<Vec<ConversionJobResponse>>, AppError> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let jobs = sf_db::queries::conversion_jobs::list_conversion_jobs(
        &conn,
        params.status.as_deref(),
        params.offset,
        params.limit,
    )?;

    // Build item_id -> name map for all jobs.
    let mut name_map = std::collections::HashMap::new();
    for job in &jobs {
        if !name_map.contains_key(&job.item_id) {
            if let Ok(Some(item)) = sf_db::queries::items::get_item(&conn, job.item_id) {
                name_map.insert(job.item_id, item.name);
            }
        }
    }

    let responses: Vec<ConversionJobResponse> = jobs
        .iter()
        .map(|job| ConversionJobResponse::from_model(job, name_map.get(&job.item_id).cloned()))
        .collect();
    Ok(Json(responses))
}

/// GET /api/conversions/:id
#[utoipa::path(
    get,
    path = "/api/conversions/{id}",
    params(("id" = String, Path, description = "Conversion job ID")),
    responses(
        (status = 200, description = "Conversion job details", body = ConversionJobResponse),
        (status = 404, description = "Conversion job not found")
    )
)]
pub async fn get_conversion(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> Result<Json<ConversionJobResponse>, AppError> {
    let job_id: sf_core::ConversionJobId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid conversion job ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let job = sf_db::queries::conversion_jobs::get_conversion_job(&conn, job_id)?
        .ok_or_else(|| sf_core::Error::not_found("conversion_job", job_id))?;

    let item_name = sf_db::queries::items::get_item(&conn, job.item_id)?
        .map(|i| i.name);

    Ok(Json(ConversionJobResponse::from_model(&job, item_name)))
}
