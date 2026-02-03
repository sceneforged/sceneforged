//! Job management route handlers.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;

use crate::context::AppContext;
use crate::error::AppError;

/// Query parameters for listing jobs.
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
pub struct SubmitJobRequest {
    pub file_path: String,
    #[serde(default)]
    pub priority: i32,
    pub source: Option<String>,
}

/// Convert a db Job model to a JSON value.
fn job_to_json(job: &sf_db::models::Job) -> serde_json::Value {
    serde_json::json!({
        "id": job.id.to_string(),
        "file_path": job.file_path,
        "file_name": job.file_name,
        "status": job.status,
        "rule_name": job.rule_name,
        "progress": job.progress,
        "current_step": job.current_step,
        "error": job.error,
        "source": job.source,
        "retry_count": job.retry_count,
        "max_retries": job.max_retries,
        "priority": job.priority,
        "created_at": job.created_at,
        "started_at": job.started_at,
        "completed_at": job.completed_at,
    })
}

/// GET /api/jobs
pub async fn list_jobs(
    State(ctx): State<AppContext>,
    Query(params): Query<ListJobsParams>,
) -> Result<impl IntoResponse, AppError> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let jobs = sf_db::queries::jobs::list_jobs(
        &conn,
        params.status.as_deref(),
        params.offset,
        params.limit,
    )?;
    let json: Vec<serde_json::Value> = jobs.iter().map(job_to_json).collect();
    Ok(Json(json))
}

/// POST /api/jobs/submit
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

    Ok((StatusCode::CREATED, Json(job_to_json(&job))))
}

/// GET /api/jobs/:id
pub async fn get_job(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let job_id: sf_core::JobId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid job ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let job = sf_db::queries::jobs::get_job(&conn, job_id)?
        .ok_or_else(|| sf_core::Error::not_found("job", job_id))?;

    Ok(Json(job_to_json(&job)))
}

/// POST /api/jobs/:id/retry
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
