use crate::arr::create_client;
use crate::probe::check_tools;
use crate::server::AppContext;
use crate::state::{Job, JobSource};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use utoipa::ToSchema;
use uuid::Uuid;

pub fn api_routes() -> Router<AppContext> {
    Router::new()
        .route("/health", get(health))
        .route("/stats", get(stats))
        .route("/jobs", get(list_jobs))
        .route("/jobs/submit", post(submit_job))
        .route("/jobs/:id", get(get_job))
        .route("/jobs/:id/retry", post(retry_job))
        .route("/jobs/:id", delete(delete_job))
        .route("/queue", get(get_queue))
        .route("/history", get(get_history))
        .route("/rules", get(get_rules))
        .route("/arrs", get(get_arrs))
        .route("/arrs/:name/test", post(test_arr))
        .route("/tools", get(get_tools))
}

/// Health check response.
#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    /// Service status
    pub status: String,
    /// Application version
    pub version: String,
    /// Processing statistics
    pub stats: HealthStats,
}

/// Health statistics.
#[derive(Serialize, ToSchema)]
pub struct HealthStats {
    /// Total jobs processed
    pub total_processed: u64,
    /// Success rate percentage
    pub success_rate: f32,
}

/// Check API health status.
#[utoipa::path(
    get,
    path = "/api/health",
    tag = "health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse)
    )
)]
pub async fn health(State(ctx): State<AppContext>) -> impl IntoResponse {
    let stats = ctx.state.get_stats();
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        stats: HealthStats {
            total_processed: stats.total_processed,
            success_rate: stats.success_rate(),
        },
    })
}

/// Get processing statistics.
#[utoipa::path(
    get,
    path = "/api/stats",
    tag = "jobs",
    responses(
        (status = 200, description = "Processing statistics", body = super::openapi::JobStatsSchema)
    )
)]
pub async fn stats(State(ctx): State<AppContext>) -> impl IntoResponse {
    let stats = ctx.state.get_stats();
    Json(stats)
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct ListJobsQuery {
    /// Filter by status (queued, running, completed, failed, cancelled)
    pub status: Option<String>,
    /// Maximum number of results to return
    pub limit: Option<usize>,
    /// Number of results to skip
    pub offset: Option<usize>,
}

/// List active jobs.
#[utoipa::path(
    get,
    path = "/api/jobs",
    tag = "jobs",
    params(ListJobsQuery),
    responses(
        (status = 200, description = "List of jobs", body = Vec<super::openapi::JobSchema>)
    )
)]
pub async fn list_jobs(
    State(ctx): State<AppContext>,
    Query(params): Query<ListJobsQuery>,
) -> impl IntoResponse {
    let mut jobs = ctx.state.get_active_jobs();

    // Filter by status if specified
    if let Some(status) = params.status {
        jobs.retain(|j| format!("{:?}", j.status).to_lowercase() == status.to_lowercase());
    }

    // Apply pagination
    let offset = params.offset.unwrap_or(0);
    let limit = params.limit.unwrap_or(100);
    let jobs: Vec<_> = jobs.into_iter().skip(offset).take(limit).collect();

    Json(jobs)
}

/// Request to submit a new job.
#[derive(Deserialize, ToSchema)]
pub struct SubmitJobRequest {
    /// Path to the media file to process
    pub file_path: String,
}

/// Response after submitting a job.
#[derive(Serialize, ToSchema)]
pub struct SubmitJobResponse {
    /// Unique job identifier
    pub job_id: Uuid,
    /// Path to the submitted file
    pub file_path: String,
}

/// Submit a new job for processing.
#[utoipa::path(
    post,
    path = "/api/jobs/submit",
    tag = "jobs",
    request_body = SubmitJobRequest,
    responses(
        (status = 200, description = "Job submitted successfully", body = SubmitJobResponse),
        (status = 400, description = "Invalid request"),
        (status = 409, description = "Job already exists for this file")
    )
)]
pub async fn submit_job(
    State(ctx): State<AppContext>,
    Json(payload): Json<SubmitJobRequest>,
) -> Result<Json<SubmitJobResponse>, (StatusCode, String)> {
    // Validate non-empty path
    let file_path = payload.file_path.trim();
    if file_path.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "File path cannot be empty".to_string(),
        ));
    }

    let path = PathBuf::from(file_path);

    // Check file exists
    if !path.exists() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("File does not exist: {}", file_path),
        ));
    }

    // Check it's a file, not directory
    if !path.is_file() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Path is not a file: {}", file_path),
        ));
    }

    // Queue with JobSource::Api
    match ctx.state.queue_job(path, JobSource::Api) {
        Ok(job) => {
            tracing::info!("Manually submitted job {} for file: {}", job.id, file_path);
            Ok(Json(SubmitJobResponse {
                job_id: job.id,
                file_path: file_path.to_string(),
            }))
        }
        Err(e) => Err((StatusCode::CONFLICT, e.to_string())),
    }
}

/// Get a specific job by ID.
#[utoipa::path(
    get,
    path = "/api/jobs/{id}",
    tag = "jobs",
    params(
        ("id" = Uuid, Path, description = "Job ID")
    ),
    responses(
        (status = 200, description = "Job details", body = super::openapi::JobSchema),
        (status = 404, description = "Job not found")
    )
)]
pub async fn get_job(
    State(ctx): State<AppContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<Job>, StatusCode> {
    ctx.state.get_job(id).map(Json).ok_or(StatusCode::NOT_FOUND)
}

/// Retry a failed job.
#[utoipa::path(
    post,
    path = "/api/jobs/{id}/retry",
    tag = "jobs",
    params(
        ("id" = Uuid, Path, description = "Job ID")
    ),
    responses(
        (status = 200, description = "Job requeued", body = super::openapi::JobSchema),
        (status = 400, description = "Cannot retry this job")
    )
)]
pub async fn retry_job(
    State(ctx): State<AppContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<Job>, (StatusCode, String)> {
    ctx.state
        .retry_job(id)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}

/// Delete a job.
#[utoipa::path(
    delete,
    path = "/api/jobs/{id}",
    tag = "jobs",
    params(
        ("id" = Uuid, Path, description = "Job ID")
    ),
    responses(
        (status = 204, description = "Job deleted"),
        (status = 404, description = "Job not found")
    )
)]
pub async fn delete_job(State(ctx): State<AppContext>, Path(id): Path<Uuid>) -> impl IntoResponse {
    if ctx.state.delete_job(id) {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

/// Get the current job queue.
#[utoipa::path(
    get,
    path = "/api/queue",
    tag = "jobs",
    responses(
        (status = 200, description = "Current queue", body = Vec<super::openapi::JobSchema>)
    )
)]
pub async fn get_queue(State(ctx): State<AppContext>) -> impl IntoResponse {
    let queue_ids = ctx.state.get_queue();
    let jobs: Vec<_> = queue_ids
        .into_iter()
        .filter_map(|id| ctx.state.get_job(id))
        .collect();
    Json(jobs)
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct HistoryQuery {
    /// Maximum number of results to return
    pub limit: Option<usize>,
}

/// Get job history.
#[utoipa::path(
    get,
    path = "/api/history",
    tag = "jobs",
    params(HistoryQuery),
    responses(
        (status = 200, description = "Job history", body = Vec<super::openapi::JobSchema>)
    )
)]
pub async fn get_history(
    State(ctx): State<AppContext>,
    Query(params): Query<HistoryQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(100);
    let history = ctx.state.get_history(limit);
    Json(history)
}

/// Get all processing rules.
#[utoipa::path(
    get,
    path = "/api/rules",
    tag = "config",
    responses(
        (status = 200, description = "List of rules", body = Vec<super::openapi::RuleSchema>)
    )
)]
pub async fn get_rules(State(ctx): State<AppContext>) -> impl IntoResponse {
    let rules = ctx.config.rules.clone();
    Json(rules)
}

/// Arr integration status.
#[derive(Serialize, ToSchema)]
pub struct ArrStatus {
    /// Integration name
    pub name: String,
    /// Integration type (radarr/sonarr)
    #[serde(rename = "type")]
    pub arr_type: String,
    /// Server URL
    pub url: String,
    /// Whether the integration is enabled
    pub enabled: bool,
    /// Connection status
    pub status: &'static str,
}

/// Get all Arr integrations.
#[utoipa::path(
    get,
    path = "/api/arrs",
    tag = "config",
    responses(
        (status = 200, description = "List of Arr integrations", body = Vec<ArrStatus>)
    )
)]
pub async fn get_arrs(State(ctx): State<AppContext>) -> impl IntoResponse {
    let arrs: Vec<ArrStatus> = ctx
        .config
        .arrs
        .iter()
        .map(|arr| ArrStatus {
            name: arr.name.clone(),
            arr_type: format!("{:?}", arr.arr_type).to_lowercase(),
            url: arr.url.clone(),
            enabled: arr.enabled,
            status: if arr.enabled { "unknown" } else { "disabled" },
        })
        .collect();
    Json(arrs)
}

/// Arr connection test result.
#[derive(Serialize, ToSchema)]
pub struct TestResult {
    /// Whether the test succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

/// Test Arr integration connection.
#[utoipa::path(
    post,
    path = "/api/arrs/{name}/test",
    tag = "config",
    params(
        ("name" = String, Path, description = "Arr integration name")
    ),
    responses(
        (status = 200, description = "Test result", body = TestResult),
        (status = 404, description = "Arr not found")
    )
)]
pub async fn test_arr(
    State(ctx): State<AppContext>,
    Path(name): Path<String>,
) -> Result<Json<TestResult>, StatusCode> {
    let arr = ctx.config.arrs.iter().find(|a| a.name == name);

    match arr {
        Some(arr) => {
            if !arr.enabled {
                return Ok(Json(TestResult {
                    success: false,
                    error: Some("Arr integration is disabled".to_string()),
                }));
            }

            let client = create_client(arr);
            match client.test_connection().await {
                Ok(true) => Ok(Json(TestResult {
                    success: true,
                    error: None,
                })),
                Ok(false) => Ok(Json(TestResult {
                    success: false,
                    error: Some("Connection failed: API returned non-success status".to_string()),
                })),
                Err(e) => Ok(Json(TestResult {
                    success: false,
                    error: Some(format!("Connection failed: {}", e)),
                })),
            }
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// External tool status.
#[derive(Serialize, ToSchema)]
pub struct ToolStatusResponse {
    /// Tool name
    pub name: String,
    /// Whether the tool is available
    pub available: bool,
    /// Tool version if available
    pub version: Option<String>,
    /// Path to the tool
    pub path: Option<String>,
}

/// Get status of external tools.
#[utoipa::path(
    get,
    path = "/api/tools",
    tag = "tools",
    responses(
        (status = 200, description = "Tool status list", body = Vec<ToolStatusResponse>)
    )
)]
pub async fn get_tools() -> impl IntoResponse {
    let tools = check_tools();
    let response: Vec<ToolStatusResponse> = tools
        .into_iter()
        .map(|t| ToolStatusResponse {
            name: t.name,
            available: t.available,
            version: t.version,
            path: t.path.map(|p| p.display().to_string()),
        })
        .collect();
    Json(response)
}
