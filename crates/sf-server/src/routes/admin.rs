//! Admin dashboard and tools route handlers.

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::context::AppContext;
use crate::error::AppError;

/// Dashboard response containing job counts and event bus info.
#[derive(Serialize, utoipa::ToSchema)]
pub struct DashboardResponse {
    pub jobs: DashboardJobs,
    pub event_bus: DashboardEventBus,
}

/// Job count summary for the dashboard.
#[derive(Serialize, utoipa::ToSchema)]
pub struct DashboardJobs {
    pub total: usize,
    pub queued: usize,
    pub processing: usize,
}

/// Event bus summary for the dashboard.
#[derive(Serialize, utoipa::ToSchema)]
pub struct DashboardEventBus {
    pub recent_events: usize,
}

/// Profile distribution counts for library statistics.
#[derive(Serialize, utoipa::ToSchema)]
pub struct ProfileCounts {
    pub profile_a: i64,
    pub profile_b: i64,
    pub profile_c: i64,
}

/// Library-level statistics response.
#[derive(Serialize, utoipa::ToSchema)]
pub struct LibraryStatsResponse {
    pub total_items: i64,
    pub total_files: i64,
    pub storage_bytes: i64,
    pub items_by_profile: ProfileCounts,
}

/// GET /api/admin/dashboard
#[utoipa::path(
    get,
    path = "/api/admin/dashboard",
    responses(
        (status = 200, description = "Dashboard statistics", body = DashboardResponse)
    )
)]
pub async fn dashboard(State(ctx): State<AppContext>) -> Json<DashboardResponse> {
    let conn = sf_db::pool::get_conn(&ctx.db);
    let (jobs_total, jobs_queued, jobs_processing) = if let Ok(conn) = conn {
        let total = sf_db::queries::jobs::list_jobs(&conn, None, 0, 1000)
            .map(|j| j.len())
            .unwrap_or(0);
        let queued = sf_db::queries::jobs::list_jobs(&conn, Some("queued"), 0, 1000)
            .map(|j| j.len())
            .unwrap_or(0);
        let processing = sf_db::queries::jobs::list_jobs(&conn, Some("processing"), 0, 1000)
            .map(|j| j.len())
            .unwrap_or(0);
        (total, queued, processing)
    } else {
        (0, 0, 0)
    };

    Json(DashboardResponse {
        jobs: DashboardJobs {
            total: jobs_total,
            queued: jobs_queued,
            processing: jobs_processing,
        },
        event_bus: DashboardEventBus {
            recent_events: ctx.event_bus.recent_events(10).len(),
        },
    })
}

/// GET /api/admin/tools
#[utoipa::path(
    get,
    path = "/api/admin/tools",
    responses(
        (status = 200, description = "List external tool availability", body = Vec<sf_av::ToolInfo>)
    )
)]
pub async fn tools(State(ctx): State<AppContext>) -> Json<Vec<sf_av::ToolInfo>> {
    let infos = ctx.tools.check_all();
    Json(infos)
}

/// GET /api/admin/stats
#[utoipa::path(
    get,
    path = "/api/admin/stats",
    responses(
        (status = 200, description = "Library statistics with profile distribution", body = LibraryStatsResponse)
    )
)]
pub async fn stats(State(ctx): State<AppContext>) -> Result<Json<LibraryStatsResponse>, AppError> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;

    // Count total items.
    let total_items: i64 = conn
        .query_row("SELECT COUNT(*) FROM items", [], |row| row.get(0))
        .unwrap_or(0);

    // Count total files and storage.
    let total_files = sf_db::queries::media_files::count_media_files(&conn).unwrap_or(0);
    let storage_bytes = sf_db::queries::media_files::total_storage_bytes(&conn).unwrap_or(0);

    // Profile distribution.
    let profile_counts =
        sf_db::queries::media_files::count_items_by_profile(&conn).unwrap_or_default();

    let mut profile_a: i64 = 0;
    let mut profile_b: i64 = 0;
    let mut profile_c: i64 = 0;

    for (profile, count) in &profile_counts {
        match profile.as_str() {
            "A" => profile_a = *count,
            "B" => profile_b = *count,
            "C" => profile_c = *count,
            _ => {}
        }
    }

    Ok(Json(LibraryStatsResponse {
        total_items,
        total_files,
        storage_bytes,
        items_by_profile: ProfileCounts {
            profile_a,
            profile_b,
            profile_c,
        },
    }))
}
