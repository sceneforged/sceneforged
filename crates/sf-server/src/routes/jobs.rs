//! Job management route handlers.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::context::AppContext;
use crate::error::AppError;

/// Query parameters for listing jobs.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ListJobsParams {
    pub status: Option<String>,
    #[serde(default)]
    pub offset: i64,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    50
}

/// Request body for submitting a new job.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SubmitJobRequest {
    pub file_path: String,
    #[serde(default)]
    pub priority: i32,
    pub source: Option<String>,
}

/// Job response.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct JobResponse {
    pub id: String,
    pub file_path: String,
    pub file_name: String,
    pub status: String,
    pub rule_name: Option<String>,
    pub progress: f64,
    pub current_step: Option<String>,
    pub error: Option<String>,
    pub source: Option<String>,
    pub retry_count: i32,
    pub max_retries: i32,
    pub priority: i32,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

impl JobResponse {
    fn from_model(job: &sf_db::models::Job) -> Self {
        Self {
            id: job.id.to_string(),
            file_path: job.file_path.clone(),
            file_name: job.file_name.clone(),
            status: job.status.clone(),
            rule_name: job.rule_name.clone(),
            progress: job.progress,
            current_step: job.current_step.clone(),
            error: job.error.clone(),
            source: job.source.clone(),
            retry_count: job.retry_count,
            max_retries: job.max_retries,
            priority: job.priority,
            created_at: job.created_at.clone(),
            started_at: job.started_at.clone(),
            completed_at: job.completed_at.clone(),
        }
    }
}

/// GET /api/jobs
#[utoipa::path(
    get,
    path = "/api/jobs",
    params(ListJobsParams),
    responses(
        (status = 200, description = "List jobs", body = Vec<JobResponse>)
    )
)]
pub async fn list_jobs(
    State(ctx): State<AppContext>,
    Query(params): Query<ListJobsParams>,
) -> Result<Json<Vec<JobResponse>>, AppError> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let jobs = sf_db::queries::jobs::list_jobs(
        &conn,
        params.status.as_deref(),
        params.offset,
        params.limit,
    )?;
    let responses: Vec<JobResponse> = jobs.iter().map(JobResponse::from_model).collect();
    Ok(Json(responses))
}

/// POST /api/jobs/submit
#[utoipa::path(
    post,
    path = "/api/jobs/submit",
    request_body = SubmitJobRequest,
    responses(
        (status = 201, description = "Job submitted", body = JobResponse)
    )
)]
pub async fn submit_job(
    State(ctx): State<AppContext>,
    Json(payload): Json<SubmitJobRequest>,
) -> Result<impl IntoResponse, AppError> {
    if payload.file_path.is_empty() {
        return Err(sf_core::Error::Validation("file_path is required".into()).into());
    }

    let file_name = std::path::Path::new(&payload.file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let job = sf_db::queries::jobs::create_job(
        &conn,
        &payload.file_path,
        &file_name,
        payload.source.as_deref(),
        payload.priority,
    )?;

    ctx.event_bus.broadcast(
        sf_core::events::EventCategory::Admin,
        sf_core::events::EventPayload::JobQueued { job_id: job.id },
    );

    Ok((StatusCode::CREATED, Json(JobResponse::from_model(&job))))
}

/// GET /api/jobs/:id
#[utoipa::path(
    get,
    path = "/api/jobs/{id}",
    params(("id" = String, Path, description = "Job ID")),
    responses(
        (status = 200, description = "Job details", body = JobResponse),
        (status = 404, description = "Job not found")
    )
)]
pub async fn get_job(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> Result<Json<JobResponse>, AppError> {
    let job_id: sf_core::JobId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid job ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let job = sf_db::queries::jobs::get_job(&conn, job_id)?
        .ok_or_else(|| sf_core::Error::not_found("job", job_id))?;

    Ok(Json(JobResponse::from_model(&job)))
}

/// POST /api/jobs/:id/retry
#[utoipa::path(
    post,
    path = "/api/jobs/{id}/retry",
    params(("id" = String, Path, description = "Job ID")),
    responses(
        (status = 200, description = "Job retried")
    )
)]
pub async fn retry_job(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let job_id: sf_core::JobId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid job ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let retried = sf_db::queries::jobs::retry_job(&conn, job_id)?;

    if !retried {
        return Err(sf_core::Error::Validation(
            "Job cannot be retried (not failed or max retries reached)".into(),
        )
        .into());
    }

    ctx.event_bus.broadcast(
        sf_core::events::EventCategory::Admin,
        sf_core::events::EventPayload::JobQueued { job_id },
    );

    Ok(Json(serde_json::json!({"status": "retried"})))
}

/// DELETE /api/jobs/:id
#[utoipa::path(
    delete,
    path = "/api/jobs/{id}",
    params(("id" = String, Path, description = "Job ID")),
    responses(
        (status = 204, description = "Job deleted")
    )
)]
pub async fn delete_job(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let job_id: sf_core::JobId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid job ID".into()))?;

    // Mark as failed with a cancellation message.
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let _ = sf_db::queries::jobs::fail_job(&conn, job_id, "Cancelled by user")?;

    Ok(StatusCode::NO_CONTENT)
}
