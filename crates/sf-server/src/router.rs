//! Axum router construction.
//!
//! Builds the full application router with all route groups, middleware
//! layers, and static file serving.

use axum::middleware;
use axum::routing::{delete, get, post, put};
use axum::Router;
use std::path::PathBuf;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::context::AppContext;
use crate::middleware::auth::auth_middleware;
use crate::middleware::request_id::request_id_middleware;
use crate::routes;

/// Build the complete Axum router.
pub fn build_router(ctx: AppContext, static_dir: Option<PathBuf>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Auth routes -- always accessible.
    let auth_routes = Router::new()
        .route("/auth/login", post(routes::auth::login))
        .route("/auth/logout", post(routes::auth::logout))
        .route("/auth/status", get(routes::auth::auth_status));

    // Protected API routes.
    let protected_routes = Router::new()
        // Libraries
        .route("/libraries", get(routes::libraries::list_libraries))
        .route("/libraries", post(routes::libraries::create_library))
        .route("/libraries/{id}", get(routes::libraries::get_library))
        .route("/libraries/{id}", delete(routes::libraries::delete_library))
        .route(
            "/libraries/{id}/scan",
            post(routes::libraries::scan_library),
        )
        // Items
        .route("/items", get(routes::items::list_items))
        .route("/items/{id}", get(routes::items::get_item))
        // Jobs
        .route("/jobs", get(routes::jobs::list_jobs))
        .route("/jobs/submit", post(routes::jobs::submit_job))
        .route("/jobs/{id}", get(routes::jobs::get_job))
        .route("/jobs/{id}/retry", post(routes::jobs::retry_job))
        .route("/jobs/{id}", delete(routes::jobs::delete_job))
        // SSE Events
        .route("/events", get(routes::events::events_handler))
        // Config
        .route("/config/rules", get(routes::config::get_rules))
        .route("/config/rules", put(routes::config::put_rules))
        .route("/config/arrs", get(routes::config::get_arrs))
        // Streaming
        .route(
            "/stream/hls/{item_id}/master.m3u8",
            get(routes::stream::master_playlist),
        )
        .route(
            "/stream/hls/{item_id}/{segment}",
            get(routes::stream::hls_segment),
        )
        // Images
        .route(
            "/images/{item_id}/{type}/{size}",
            get(routes::images::get_image),
        )
        // Admin
        .route("/admin/dashboard", get(routes::admin::dashboard))
        .route("/admin/tools", get(routes::admin::tools));

    // Apply auth middleware to protected routes if auth is enabled.
    let protected_routes = if ctx.config.auth.enabled {
        protected_routes.layer(middleware::from_fn_with_state(ctx.clone(), auth_middleware))
    } else {
        protected_routes
    };

    // Combine auth and protected under /api.
    let api = auth_routes.merge(protected_routes);

    let mut app = Router::new()
        .route("/health", get(routes::health::health_check))
        .nest("/api", api)
        .route(
            "/webhook/{arr_name}",
            post(routes::webhook::handle_webhook),
        )
        .route("/metrics", get(routes::metrics::metrics_handler))
        .layer(middleware::from_fn(request_id_middleware))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(ctx);

    // Static file serving for UI build.
    if let Some(dir) = static_dir {
        if dir.exists() {
            tracing::info!("Serving static files from {:?}", dir);
            let index_path = dir.join("index.html");
            app = app.fallback_service(
                tower_http::services::ServeDir::new(&dir)
                    .append_index_html_on_directories(true)
                    .not_found_service(tower_http::services::ServeFile::new(index_path)),
            );
        }
    }

    app
}
