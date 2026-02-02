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

async fn health(State(ctx): State<AppContext>) -> impl IntoResponse {
    let stats = ctx.state.get_stats();
    Json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
        "stats": {
            "total_processed": stats.total_processed,
            "success_rate": stats.success_rate()
        }
    }))
}

async fn stats(State(ctx): State<AppContext>) -> impl IntoResponse {
    let stats = ctx.state.get_stats();
    Json(stats)
}

#[derive(Deserialize)]
struct ListJobsQuery {
    status: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn list_jobs(
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

#[derive(Deserialize)]
struct SubmitJobRequest {
    file_path: String,
}

#[derive(Serialize)]
struct SubmitJobResponse {
    job_id: Uuid,
    file_path: String,
}

async fn submit_job(
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

async fn get_job(
    State(ctx): State<AppContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<Job>, StatusCode> {
    ctx.state.get_job(id).map(Json).ok_or(StatusCode::NOT_FOUND)
}

async fn retry_job(
    State(ctx): State<AppContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<Job>, (StatusCode, String)> {
    ctx.state
        .retry_job(id)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}

async fn delete_job(State(ctx): State<AppContext>, Path(id): Path<Uuid>) -> impl IntoResponse {
    if ctx.state.delete_job(id) {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn get_queue(State(ctx): State<AppContext>) -> impl IntoResponse {
    let queue_ids = ctx.state.get_queue();
    let jobs: Vec<_> = queue_ids
        .into_iter()
        .filter_map(|id| ctx.state.get_job(id))
        .collect();
    Json(jobs)
}

#[derive(Deserialize)]
struct HistoryQuery {
    limit: Option<usize>,
}

async fn get_history(
    State(ctx): State<AppContext>,
    Query(params): Query<HistoryQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(100);
    let history = ctx.state.get_history(limit);
    Json(history)
}

async fn get_rules(State(ctx): State<AppContext>) -> impl IntoResponse {
    let rules = ctx.config.rules.clone();
    Json(rules)
}

#[derive(Serialize)]
struct ArrStatus {
    name: String,
    #[serde(rename = "type")]
    arr_type: String,
    url: String,
    enabled: bool,
    status: &'static str,
}

async fn get_arrs(State(ctx): State<AppContext>) -> impl IntoResponse {
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

#[derive(Serialize)]
struct TestResult {
    success: bool,
    error: Option<String>,
}

async fn test_arr(
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

#[derive(Serialize)]
struct ToolStatusResponse {
    name: String,
    available: bool,
    version: Option<String>,
    path: Option<String>,
}

async fn get_tools() -> impl IntoResponse {
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
