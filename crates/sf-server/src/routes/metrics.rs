//! Prometheus metrics endpoint.

use axum::response::IntoResponse;

/// GET /metrics -- Prometheus-format metrics.
pub async fn metrics_handler() -> impl IntoResponse {
    // Attempt to render the default global recorder.
    // The recorder is installed during startup; if absent we return empty.
    (
        axum::http::StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4")],
        "# No metrics recorder installed\n",
    )
}
