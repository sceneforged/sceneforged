//! Admin dashboard and tools route handlers.

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

use crate::context::AppContext;

/// GET /api/admin/dashboard
pub async fn dashboard(State(ctx): State<AppContext>) -> impl IntoResponse {
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

    Json(serde_json::json!({
        "jobs": {
            "total": jobs_total,
            "queued": jobs_queued,
            "processing": jobs_processing,
        },
        "event_bus": {
            "recent_events": ctx.event_bus.recent_events(10).len(),
        }
    }))
}

/// GET /api/admin/tools
pub async fn tools(State(ctx): State<AppContext>) -> impl IntoResponse {
    let infos = ctx.tools.check_all();
    Json(serde_json::to_value(&infos).unwrap_or_default())
}
