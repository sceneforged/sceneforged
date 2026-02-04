//! Admin dashboard and tools route handlers.

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::context::AppContext;

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
