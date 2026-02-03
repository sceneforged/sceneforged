//! Admin API routes for dashboard statistics and management.
//!
//! These routes provide administrative functionality including:
//! - Dashboard statistics and overview
//! - Active streaming session monitoring
//! - Library statistics
//! - Conversion management (single and batch)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use sceneforged_common::{ItemId, Profile};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::AppContext;

/// Create admin routes.
pub fn admin_routes() -> Router<AppContext> {
    Router::new()
        .route("/admin/dashboard", get(get_dashboard))
        .route("/admin/streams", get(get_streams))
        .route("/admin/stats", get(get_library_stats))
        .route("/admin/conversion-jobs", get(list_conversion_jobs))
        .route(
            "/admin/conversion-jobs/:job_id",
            axum::routing::delete(cancel_conversion_job),
        )
        .route("/items/:item_id/conversion", get(get_item_conversion))
        .route("/items/:item_id/convert", post(convert_item))
        .route("/conversions/batch", post(batch_convert))
        .route("/conversions/dv-batch", post(batch_dv_convert))
}

// ============================================================================
// Request/Response types
// ============================================================================

/// Dashboard overview combining stats, streams, and queue.
#[derive(Debug, Serialize, ToSchema)]
pub struct DashboardResponse {
    /// Library statistics
    pub stats: LibraryStatsResponse,
    /// Active streaming sessions
    pub streams: Vec<StreamSessionResponse>,
    /// Job queue summary
    pub queue: QueueSummaryResponse,
}

/// Library statistics.
#[derive(Debug, Serialize, ToSchema)]
pub struct LibraryStatsResponse {
    /// Total number of items
    pub total_items: u32,
    /// Total number of media files
    pub total_files: u32,
    /// Total storage used in bytes
    pub storage_bytes: i64,
    /// Items by profile (A, B, C counts)
    pub items_by_profile: ProfileCountsResponse,
}

/// Profile counts.
#[derive(Debug, Serialize, ToSchema)]
pub struct ProfileCountsResponse {
    /// Items with Profile A
    pub profile_a: u32,
    /// Items with Profile B
    pub profile_b: u32,
    /// Items with Profile C
    pub profile_c: u32,
}

/// Active streaming session.
#[derive(Debug, Serialize, ToSchema)]
pub struct StreamSessionResponse {
    /// Session ID
    pub id: String,
    /// Client IP address
    pub client_ip: String,
    /// Media item ID being streamed
    pub item_id: i64,
    /// Profile being served
    pub profile: String,
    /// Session start time
    pub started_at: String,
    /// Duration in seconds
    pub duration_seconds: i64,
}

/// Job queue summary.
#[derive(Debug, Serialize, ToSchema)]
pub struct QueueSummaryResponse {
    /// Number of jobs in queue
    pub queued: usize,
    /// Number of running jobs
    pub running: usize,
}

/// Conversion options for an item.
#[derive(Debug, Serialize, ToSchema)]
pub struct ConversionOptionsResponse {
    /// Profiles that currently exist
    pub current_profiles: Vec<String>,
    /// Profiles that can be created via conversion
    pub viable_targets: Vec<String>,
}

/// Request to convert an item.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ConvertItemRequest {
    /// Target profiles to create
    pub target_profiles: Vec<String>,
}

/// Response after starting conversion.
#[derive(Debug, Serialize, ToSchema)]
pub struct ConvertItemResponse {
    /// Created job IDs
    pub job_ids: Vec<String>,
}

/// Request to batch convert items.
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchConvertRequest {
    /// Item IDs to convert
    pub item_ids: Vec<String>,
    /// Target profile
    pub target_profile: String,
}

/// Response after starting batch conversion.
#[derive(Debug, Serialize, ToSchema)]
pub struct BatchConvertResponse {
    /// Created job IDs
    pub job_ids: Vec<String>,
}

/// Request to batch convert DV Profile 7 items to Profile 8.
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchDvConvertRequest {
    /// Item IDs to convert
    pub item_ids: Vec<String>,
}

/// Response after starting DV batch conversion.
#[derive(Debug, Serialize, ToSchema)]
pub struct BatchDvConvertResponse {
    /// Created job IDs
    pub job_ids: Vec<String>,
}

// ============================================================================
// Handlers
// ============================================================================

/// Get dashboard overview.
#[utoipa::path(
    get,
    path = "/api/admin/dashboard",
    tag = "admin",
    responses(
        (status = 200, description = "Dashboard overview", body = DashboardResponse),
        (status = 503, description = "Database not available")
    )
)]
pub async fn get_dashboard(State(ctx): State<AppContext>) -> impl IntoResponse {
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

    // Get library stats
    let stats = match compute_library_stats(&conn) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    // Get active streams
    let streams = if let Some(ref session_manager) = ctx.session_manager {
        session_manager
            .list_active_sessions()
            .into_iter()
            .map(|s| StreamSessionResponse {
                id: s.id,
                client_ip: s.client_ip,
                item_id: s.item_id,
                profile: s.profile.to_string(),
                started_at: s.started_at.to_rfc3339(),
                duration_seconds: (chrono::Utc::now() - s.started_at).num_seconds(),
            })
            .collect()
    } else {
        vec![]
    };

    // Get queue summary from both in-memory state and database conversion jobs
    let active_jobs = ctx.state.get_active_jobs();
    let mut queued_count = active_jobs
        .iter()
        .filter(|j| matches!(j.status, crate::state::JobStatus::Queued))
        .count();
    let mut running_count = active_jobs
        .iter()
        .filter(|j| matches!(j.status, crate::state::JobStatus::Running))
        .count();

    // Also count database conversion jobs
    if let Ok(db_jobs) = sceneforged_db::queries::conversion_jobs::list_active_jobs(&conn, 1000) {
        for job in &db_jobs {
            match job.status {
                sceneforged_db::models::ConversionStatus::Queued => queued_count += 1,
                sceneforged_db::models::ConversionStatus::Running => running_count += 1,
                _ => {}
            }
        }
    }

    let queue_summary = QueueSummaryResponse {
        queued: queued_count,
        running: running_count,
    };

    let response = DashboardResponse {
        stats,
        streams,
        queue: queue_summary,
    };

    Json(response).into_response()
}

/// Get active streaming sessions.
#[utoipa::path(
    get,
    path = "/api/admin/streams",
    tag = "admin",
    responses(
        (status = 200, description = "List of active streams", body = Vec<StreamSessionResponse>)
    )
)]
pub async fn get_streams(State(ctx): State<AppContext>) -> impl IntoResponse {
    let sessions = if let Some(ref session_manager) = ctx.session_manager {
        session_manager
            .list_active_sessions()
            .into_iter()
            .map(|s| StreamSessionResponse {
                id: s.id,
                client_ip: s.client_ip,
                item_id: s.item_id,
                profile: s.profile.to_string(),
                started_at: s.started_at.to_rfc3339(),
                duration_seconds: (chrono::Utc::now() - s.started_at).num_seconds(),
            })
            .collect()
    } else {
        vec![]
    };

    Json(sessions)
}

/// Get library statistics.
#[utoipa::path(
    get,
    path = "/api/admin/stats",
    tag = "admin",
    responses(
        (status = 200, description = "Library statistics", body = LibraryStatsResponse),
        (status = 503, description = "Database not available")
    )
)]
pub async fn get_library_stats(State(ctx): State<AppContext>) -> impl IntoResponse {
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

    match compute_library_stats(&conn) {
        Ok(stats) => Json(stats).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Get conversion options for an item.
#[utoipa::path(
    get,
    path = "/api/items/{item_id}/conversion",
    tag = "admin",
    params(
        ("item_id" = String, Path, description = "Item ID")
    ),
    responses(
        (status = 200, description = "Conversion options", body = ConversionOptionsResponse),
        (status = 400, description = "Invalid item ID"),
        (status = 503, description = "Database or conversion manager not available")
    )
)]
pub async fn get_item_conversion(
    State(ctx): State<AppContext>,
    Path(item_id): Path<String>,
) -> impl IntoResponse {
    let Some(ref conversion_manager) = ctx.conversion_manager else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Conversion manager not available"})),
        )
            .into_response();
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

    match conversion_manager.get_conversion_options(id) {
        Ok(options) => Json(ConversionOptionsResponse {
            current_profiles: options
                .current_profiles
                .into_iter()
                .map(|p| p.to_string())
                .collect(),
            viable_targets: options
                .viable_targets
                .into_iter()
                .map(|p| p.to_string())
                .collect(),
        })
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Start conversion for an item.
#[utoipa::path(
    post,
    path = "/api/items/{item_id}/convert",
    tag = "admin",
    params(
        ("item_id" = String, Path, description = "Item ID")
    ),
    request_body = ConvertItemRequest,
    responses(
        (status = 200, description = "Conversion jobs created", body = ConvertItemResponse),
        (status = 400, description = "Invalid request"),
        (status = 503, description = "Conversion manager not available")
    )
)]
pub async fn convert_item(
    State(ctx): State<AppContext>,
    Path(item_id): Path<String>,
    Json(request): Json<ConvertItemRequest>,
) -> impl IntoResponse {
    let Some(ref conversion_manager) = ctx.conversion_manager else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Conversion manager not available"})),
        )
            .into_response();
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

    // Parse profiles
    let profiles: Result<Vec<Profile>, _> = request
        .target_profiles
        .iter()
        .map(|s| s.parse::<Profile>())
        .collect();

    let profiles = match profiles {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Invalid profile: {}", e)})),
            )
                .into_response()
        }
    };

    match conversion_manager.start_conversion(id, profiles) {
        Ok(job_ids) => {
            // Broadcast SSE events for each created job
            for job_id in &job_ids {
                ctx.state.broadcast(crate::state::AppEvent::conversion_job_created(
                    job_id.clone(),
                    item_id.clone(),
                    "queued".to_string(),
                ));
            }
            Json(ConvertItemResponse { job_ids }).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Batch convert multiple items.
#[utoipa::path(
    post,
    path = "/api/conversions/batch",
    tag = "admin",
    request_body = BatchConvertRequest,
    responses(
        (status = 200, description = "Batch conversion jobs created", body = BatchConvertResponse),
        (status = 400, description = "Invalid request"),
        (status = 503, description = "Conversion manager not available")
    )
)]
pub async fn batch_convert(
    State(ctx): State<AppContext>,
    Json(request): Json<BatchConvertRequest>,
) -> impl IntoResponse {
    let Some(ref conversion_manager) = ctx.conversion_manager else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Conversion manager not available"})),
        )
            .into_response();
    };

    // Parse item IDs
    let item_ids: Result<Vec<ItemId>, _> = request
        .item_ids
        .iter()
        .map(|s| s.parse::<uuid::Uuid>().map(ItemId::from))
        .collect();

    let item_ids = match item_ids {
        Ok(ids) => ids,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid item ID format"})),
            )
                .into_response()
        }
    };

    // Parse profile
    let profile = match request.target_profile.parse::<Profile>() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Invalid profile: {}", e)})),
            )
                .into_response()
        }
    };

    match conversion_manager.batch_convert(item_ids, profile) {
        Ok(job_ids) => {
            // Broadcast SSE event for each created job
            for job_id in &job_ids {
                ctx.state.broadcast(crate::state::AppEvent::conversion_job_created(
                    job_id.clone(),
                    String::new(),
                    "queued".to_string(),
                ));
            }
            Json(BatchConvertResponse { job_ids }).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Batch convert DV Profile 7 items to Profile 8.
#[utoipa::path(
    post,
    path = "/api/conversions/dv-batch",
    tag = "admin",
    request_body = BatchDvConvertRequest,
    responses(
        (status = 200, description = "DV batch conversion jobs created", body = BatchDvConvertResponse),
        (status = 400, description = "Invalid request"),
        (status = 503, description = "Conversion manager not available")
    )
)]
pub async fn batch_dv_convert(
    State(ctx): State<AppContext>,
    Json(request): Json<BatchDvConvertRequest>,
) -> impl IntoResponse {
    let Some(ref conversion_manager) = ctx.conversion_manager else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Conversion manager not available"})),
        )
            .into_response();
    };

    // Parse item IDs, filtering out invalid ones
    let item_ids: Vec<ItemId> = request
        .item_ids
        .iter()
        .filter_map(|s| s.parse::<uuid::Uuid>().ok().map(ItemId::from))
        .collect();

    match conversion_manager.batch_dv_convert(item_ids) {
        Ok(job_ids) => Json(BatchDvConvertResponse { job_ids }).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Conversion job info for API responses.
#[derive(Debug, Serialize, ToSchema)]
pub struct ConversionJobResponse {
    /// Job ID
    pub id: String,
    /// Item ID this job is for
    pub item_id: String,
    /// Source file ID
    pub source_file_id: String,
    /// Current status (queued, running, completed, failed, cancelled)
    pub status: String,
    /// Progress percentage (0-100)
    pub progress_pct: f64,
    /// Output file path
    pub output_path: Option<String>,
    /// Error message if failed
    pub error_message: Option<String>,
    /// When the job was created
    pub created_at: String,
    /// When the job started
    pub started_at: Option<String>,
    /// When the job completed
    pub completed_at: Option<String>,
}

/// List conversion jobs.
#[utoipa::path(
    get,
    path = "/api/admin/conversion-jobs",
    tag = "admin",
    responses(
        (status = 200, description = "List of conversion jobs", body = Vec<ConversionJobResponse>),
        (status = 503, description = "Database not available")
    )
)]
pub async fn list_conversion_jobs(State(ctx): State<AppContext>) -> impl IntoResponse {
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

    match sceneforged_db::queries::conversion_jobs::list_active_jobs(&conn, 100) {
        Ok(jobs) => {
            let response: Vec<ConversionJobResponse> = jobs
                .into_iter()
                .map(|j| ConversionJobResponse {
                    id: j.id,
                    item_id: j.item_id.to_string(),
                    source_file_id: j.source_file_id.to_string(),
                    status: j.status.to_string(),
                    progress_pct: j.progress_pct,
                    output_path: j.output_path,
                    error_message: j.error_message,
                    created_at: j.created_at.to_rfc3339(),
                    started_at: j.started_at.map(|t| t.to_rfc3339()),
                    completed_at: j.completed_at.map(|t| t.to_rfc3339()),
                })
                .collect();
            Json(response).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Cancel a conversion job.
#[utoipa::path(
    delete,
    path = "/api/admin/conversion-jobs/{job_id}",
    tag = "admin",
    params(
        ("job_id" = String, Path, description = "Conversion job ID")
    ),
    responses(
        (status = 204, description = "Job cancelled"),
        (status = 404, description = "Job not found or not cancellable"),
        (status = 503, description = "Database not available")
    )
)]
pub async fn cancel_conversion_job(
    State(ctx): State<AppContext>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    let Some(ref pool) = ctx.db_pool else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    match sceneforged_db::queries::conversion_jobs::cancel_job(&conn, &job_id) {
        Ok(()) => {
            ctx.state.broadcast(crate::state::AppEvent::conversion_job_cancelled(
                job_id,
                String::new(), // item_id not easily available here
            ));
            StatusCode::NO_CONTENT.into_response()
        }
        Err(_) => StatusCode::NO_CONTENT.into_response(), // Idempotent
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Compute library statistics from the database.
fn compute_library_stats(
    conn: &rusqlite::Connection,
) -> Result<LibraryStatsResponse, rusqlite::Error> {
    // Total items
    let total_items: u32 = conn.query_row("SELECT COUNT(*) FROM items", [], |row| row.get(0))?;

    // Total files
    let total_files: u32 = conn.query_row("SELECT COUNT(*) FROM media_files", [], |row| row.get(0))?;

    // Total storage
    let storage_bytes: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(file_size), 0) FROM media_files",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Items by profile - count distinct items that have at least one file of each profile
    let profile_a: u32 = conn
        .query_row(
            "SELECT COUNT(DISTINCT item_id) FROM media_files WHERE profile = 'A'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let profile_b: u32 = conn
        .query_row(
            "SELECT COUNT(DISTINCT item_id) FROM media_files WHERE profile = 'B'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let profile_c: u32 = conn
        .query_row(
            "SELECT COUNT(DISTINCT item_id) FROM media_files WHERE profile = 'C'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(LibraryStatsResponse {
        total_items,
        total_files,
        storage_bytes,
        items_by_profile: ProfileCountsResponse {
            profile_a,
            profile_b,
            profile_c,
        },
    })
}
