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
    pub source_video_codec: Option<String>,
    pub source_audio_codec: Option<String>,
    pub source_resolution: Option<String>,
    pub source_container: Option<String>,
    pub status: String,
    pub progress_pct: f64,
    pub encode_fps: Option<f64>,
    pub eta_secs: Option<i64>,
    pub error: Option<String>,
    pub priority: i32,
    pub bitrate: Option<String>,
    pub speed: Option<String>,
    pub output_size: Option<i64>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

impl ConversionJobResponse {
    fn from_model(
        job: &sf_db::models::ConversionJob,
        item_name: Option<String>,
        source_mf: Option<&sf_db::models::MediaFile>,
    ) -> Self {
        let source_resolution = source_mf.and_then(|mf| {
            match (mf.resolution_width, mf.resolution_height) {
                (Some(w), Some(h)) => Some(format!("{w}x{h}")),
                _ => None,
            }
        });

        Self {
            id: job.id.to_string(),
            item_id: job.item_id.to_string(),
            item_name,
            media_file_id: job.media_file_id.map(|id| id.to_string()),
            source_media_file_id: job.source_media_file_id.map(|id| id.to_string()),
            source_video_codec: source_mf.and_then(|mf| mf.video_codec.clone()),
            source_audio_codec: source_mf.and_then(|mf| mf.audio_codec.clone()),
            source_resolution,
            source_container: source_mf.and_then(|mf| mf.container.clone()),
            status: job.status.clone(),
            progress_pct: job.progress_pct,
            encode_fps: job.encode_fps,
            eta_secs: job.eta_secs,
            error: job.error.clone(),
            priority: job.priority,
            bitrate: job.bitrate.clone(),
            speed: job.speed.clone(),
            output_size: job.output_size,
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
    let source_mf = if let Some(ref mf_id_str) = payload.media_file_id {
        let mf_id = mf_id_str
            .parse::<sf_core::MediaFileId>()
            .map_err(|_| sf_core::Error::Validation("Invalid media_file_id".into()))?;
        sf_db::queries::media_files::get_media_file(&conn, mf_id)?
            .ok_or_else(|| sf_core::Error::not_found("media_file", mf_id))?
    } else {
        let files = sf_db::queries::media_files::list_media_files_by_item(&conn, item_id)?;
        let idx = files.iter().position(|f| f.role == "source").unwrap_or(0);
        files.into_iter().nth(idx).ok_or_else(|| {
            sf_core::Error::Validation("No media files found for item".into())
        })?
    };

    let job = sf_db::queries::conversion_jobs::create_conversion_job(&conn, item_id, source_mf.id)?;

    let item_name = sf_db::queries::items::get_item(&conn, item_id)?
        .map(|i| i.name);

    ctx.event_bus.broadcast(
        sf_core::events::EventCategory::Admin,
        sf_core::events::EventPayload::ConversionQueued { job_id: job.id },
    );

    Ok((
        StatusCode::CREATED,
        Json(ConversionJobResponse::from_model(&job, item_name, Some(&source_mf))),
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

    // Build item_id -> name map and source_media_file_id -> MediaFile map.
    let mut name_map = std::collections::HashMap::new();
    let mut source_mf_map = std::collections::HashMap::new();
    for job in &jobs {
        if !name_map.contains_key(&job.item_id) {
            if let Ok(Some(item)) = sf_db::queries::items::get_item(&conn, job.item_id) {
                name_map.insert(job.item_id, item.name);
            }
        }
        if let Some(smf_id) = job.source_media_file_id {
            if !source_mf_map.contains_key(&smf_id) {
                if let Ok(Some(mf)) = sf_db::queries::media_files::get_media_file(&conn, smf_id) {
                    source_mf_map.insert(smf_id, mf);
                }
            }
        }
    }

    let responses: Vec<ConversionJobResponse> = jobs
        .iter()
        .map(|job| {
            let source_mf = job.source_media_file_id.and_then(|id| source_mf_map.get(&id));
            ConversionJobResponse::from_model(job, name_map.get(&job.item_id).cloned(), source_mf)
        })
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

    let source_mf = job.source_media_file_id
        .and_then(|id| sf_db::queries::media_files::get_media_file(&conn, id).ok().flatten());

    Ok(Json(ConversionJobResponse::from_model(&job, item_name, source_mf.as_ref())))
}

/// Request body for batch conversion.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct BatchConvertRequest {
    pub item_ids: Vec<String>,
}

/// Response for batch conversion.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct BatchConvertResponse {
    pub job_ids: Vec<String>,
    pub errors: Vec<String>,
}

/// POST /api/conversions/batch
///
/// Create a Profile B conversion job for each of the given items.
#[utoipa::path(
    post,
    path = "/api/conversions/batch",
    request_body = BatchConvertRequest,
    responses(
        (status = 200, description = "Batch conversion submitted", body = BatchConvertResponse)
    )
)]
pub async fn batch_convert(
    State(ctx): State<AppContext>,
    Json(payload): Json<BatchConvertRequest>,
) -> Result<Json<BatchConvertResponse>, AppError> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let mut job_ids = Vec::new();
    let mut errors = Vec::new();

    for id_str in &payload.item_ids {
        let item_id: sf_core::ItemId = match id_str.parse() {
            Ok(id) => id,
            Err(_) => {
                errors.push(format!("Invalid item_id: {id_str}"));
                continue;
            }
        };

        // Skip items that already have active conversions.
        match sf_db::queries::conversion_jobs::has_active_conversion_for_item(&conn, item_id) {
            Ok(true) => {
                errors.push(format!("Item {id_str} already has an active conversion"));
                continue;
            }
            Err(e) => {
                errors.push(format!("Error checking item {id_str}: {e}"));
                continue;
            }
            Ok(false) => {}
        }

        // Find the source media file.
        let files = match sf_db::queries::media_files::list_media_files_by_item(&conn, item_id) {
            Ok(f) => f,
            Err(e) => {
                errors.push(format!("Error listing files for {id_str}: {e}"));
                continue;
            }
        };

        let source = match files.iter().find(|f| f.role == "source").or(files.first()) {
            Some(s) => s,
            None => {
                errors.push(format!("No media files for item {id_str}"));
                continue;
            }
        };

        match sf_db::queries::conversion_jobs::create_conversion_job(&conn, item_id, source.id) {
            Ok(job) => {
                ctx.event_bus.broadcast(
                    sf_core::events::EventCategory::Admin,
                    sf_core::events::EventPayload::ConversionQueued { job_id: job.id },
                );
                job_ids.push(job.id.to_string());
            }
            Err(e) => {
                errors.push(format!("Error creating job for {id_str}: {e}"));
            }
        }
    }

    Ok(Json(BatchConvertResponse { job_ids, errors }))
}

/// Request body for DV batch conversion.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct DvBatchConvertRequest {
    pub item_ids: Vec<String>,
}

/// POST /api/conversions/dv-batch
///
/// Create DV Profile 7 -> Profile 8 conversion jobs for the given items.
/// Only items with DV Profile 7 source files will have jobs created.
#[utoipa::path(
    post,
    path = "/api/conversions/dv-batch",
    request_body = DvBatchConvertRequest,
    responses(
        (status = 200, description = "DV batch conversion submitted", body = BatchConvertResponse)
    )
)]
pub async fn dv_batch_convert(
    State(ctx): State<AppContext>,
    Json(payload): Json<DvBatchConvertRequest>,
) -> Result<Json<BatchConvertResponse>, AppError> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let mut job_ids = Vec::new();
    let mut errors = Vec::new();

    for id_str in &payload.item_ids {
        let item_id: sf_core::ItemId = match id_str.parse() {
            Ok(id) => id,
            Err(_) => {
                errors.push(format!("Invalid item_id: {id_str}"));
                continue;
            }
        };

        // Skip items that already have active conversions.
        match sf_db::queries::conversion_jobs::has_active_conversion_for_item(&conn, item_id) {
            Ok(true) => {
                errors.push(format!("Item {id_str} already has an active conversion"));
                continue;
            }
            Err(e) => {
                errors.push(format!("Error checking item {id_str}: {e}"));
                continue;
            }
            Ok(false) => {}
        }

        // Find a DV Profile 7 source media file.
        let files = match sf_db::queries::media_files::list_media_files_by_item(&conn, item_id) {
            Ok(f) => f,
            Err(e) => {
                errors.push(format!("Error listing files for {id_str}: {e}"));
                continue;
            }
        };

        let dv7_source = files.iter().find(|f| f.has_dolby_vision && f.dv_profile == Some(7));

        let source = match dv7_source {
            Some(s) => s,
            None => {
                errors.push(format!("No DV Profile 7 file for item {id_str}"));
                continue;
            }
        };

        match sf_db::queries::conversion_jobs::create_conversion_job(&conn, item_id, source.id) {
            Ok(job) => {
                ctx.event_bus.broadcast(
                    sf_core::events::EventCategory::Admin,
                    sf_core::events::EventPayload::ConversionQueued { job_id: job.id },
                );
                job_ids.push(job.id.to_string());
            }
            Err(e) => {
                errors.push(format!("Error creating job for {id_str}: {e}"));
            }
        }
    }

    Ok(Json(BatchConvertResponse { job_ids, errors }))
}

/// DELETE /api/conversions/:id
///
/// Cancel or delete a conversion job. Queued/failed jobs are deleted;
/// processing jobs are cancelled (marked as failed).
#[utoipa::path(
    delete,
    path = "/api/conversions/{id}",
    params(("id" = String, Path, description = "Conversion job ID")),
    responses(
        (status = 200, description = "Conversion job cancelled/deleted"),
        (status = 404, description = "Conversion job not found or not cancellable")
    )
)]
pub async fn delete_conversion(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let job_id: sf_core::ConversionJobId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid conversion job ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;

    let job = sf_db::queries::conversion_jobs::get_conversion_job(&conn, job_id)?
        .ok_or_else(|| sf_core::Error::not_found("conversion_job", job_id))?;

    let removed = match job.status.as_str() {
        "queued" | "failed" => {
            sf_db::queries::conversion_jobs::delete_conversion_job(&conn, job_id)?
        }
        "processing" => {
            let cancelled = sf_db::queries::conversion_jobs::cancel_conversion_job(&conn, job_id)?;
            // Trigger the cancellation token to kill the running ffmpeg process.
            if cancelled {
                if let Some((_, token)) = ctx.active_conversions.remove(&job_id) {
                    token.cancel();
                }
            }
            cancelled
        }
        _ => false,
    };

    if !removed {
        return Err(sf_core::Error::Validation(
            "Conversion job cannot be cancelled in its current state".into(),
        )
        .into());
    }

    ctx.event_bus.broadcast(
        sf_core::events::EventCategory::Admin,
        sf_core::events::EventPayload::ConversionFailed {
            job_id,
            error: "Cancelled by user".into(),
        },
    );

    Ok(StatusCode::OK)
}

/// Request body for reordering the queue.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ReorderRequest {
    pub job_ids: Vec<String>,
}

/// PUT /api/conversions/reorder
///
/// Reorder queued conversion jobs. The first ID gets the highest priority.
pub async fn reorder_conversions(
    State(ctx): State<AppContext>,
    Json(payload): Json<ReorderRequest>,
) -> Result<StatusCode, AppError> {
    let job_ids: Vec<sf_core::ConversionJobId> = payload
        .job_ids
        .iter()
        .map(|s| {
            s.parse()
                .map_err(|_| sf_core::Error::Validation(format!("Invalid job ID: {s}")))
        })
        .collect::<sf_core::Result<Vec<_>>>()?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    sf_db::queries::conversion_jobs::reorder_queue(&conn, &job_ids)?;

    Ok(StatusCode::OK)
}

/// Request body for updating priority.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdatePriorityRequest {
    pub priority: i32,
}

/// PUT /api/conversions/{id}/priority
///
/// Update the priority of a single conversion job.
pub async fn update_priority(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
    Json(payload): Json<UpdatePriorityRequest>,
) -> Result<StatusCode, AppError> {
    let job_id: sf_core::ConversionJobId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid conversion job ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let updated =
        sf_db::queries::conversion_jobs::update_conversion_priority(&conn, job_id, payload.priority)?;

    if !updated {
        return Err(sf_core::Error::not_found("conversion_job", job_id).into());
    }

    Ok(StatusCode::OK)
}
