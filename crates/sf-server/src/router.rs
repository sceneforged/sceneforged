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
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::context::AppContext;
use crate::middleware::auth::auth_middleware;
use crate::middleware::request_id::request_id_middleware;
use crate::routes;

#[derive(OpenApi)]
#[openapi(
    paths(
        routes::auth::login,
        routes::auth::logout,
        routes::auth::auth_status,
        routes::libraries::list_libraries,
        routes::libraries::create_library,
        routes::libraries::get_library,
        routes::libraries::delete_library,
        routes::libraries::scan_library,
        routes::items::list_items,
        routes::items::get_item,
        routes::items::list_children,
        routes::jobs::list_jobs,
        routes::jobs::submit_job,
        routes::jobs::get_job,
        routes::jobs::retry_job,
        routes::jobs::delete_job,
        routes::config::get_rules,
        routes::config::put_rules,
        routes::admin::dashboard,
        routes::admin::tools,
        routes::conversions::list_conversions,
        routes::conversions::submit_conversion,
        routes::conversions::get_conversion,
        routes::conversions::delete_conversion,
        routes::playback::continue_watching,
        routes::playback::get_playback,
        routes::playback::update_progress,
        routes::playback::mark_played,
        routes::playback::mark_unplayed,
        routes::playback::list_favorites,
        routes::playback::add_favorite,
        routes::playback::remove_favorite,
        routes::playback::get_user_data,
    ),
    components(schemas(
        routes::auth::LoginRequest,
        routes::auth::AuthResponse,
        routes::auth::AuthStatusResponse,
        routes::libraries::LibraryResponse,
        routes::libraries::CreateLibraryRequest,
        routes::items::ItemResponse,
        routes::items::MediaFileResponse,
        routes::items::ImageResponse,
        routes::jobs::JobResponse,
        routes::jobs::SubmitJobRequest,
        routes::conversions::ConversionJobResponse,
        routes::conversions::SubmitConversionRequest,
        routes::admin::DashboardResponse,
        routes::admin::DashboardJobs,
        routes::admin::DashboardEventBus,
        routes::playback::PlaybackResponse,
        routes::playback::UpdateProgressRequest,
        routes::playback::FavoriteResponse,
        routes::playback::UserDataResponse,
        routes::playback::ContinueWatchingEntry,
        routes::playback::FavoriteEntry,
        sf_av::ToolInfo,
    ))
)]
struct ApiDoc;

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
        .route(
            "/items/{id}/children",
            get(routes::items::list_children),
        )
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
        .route(
            "/config/arrs",
            get(routes::config::get_arrs).post(routes::config::create_arr),
        )
        .route(
            "/config/arrs/{name}",
            put(routes::config::update_arr).delete(routes::config::delete_arr),
        )
        .route(
            "/config/arrs/{name}/test",
            post(routes::config::test_arr),
        )
        .route(
            "/config/jellyfins",
            get(routes::config::get_jellyfins).post(routes::config::create_jellyfin),
        )
        .route(
            "/config/jellyfins/{name}",
            put(routes::config::update_jellyfin).delete(routes::config::delete_jellyfin),
        )
        .route(
            "/config/conversion",
            get(routes::config::get_conversion).put(routes::config::update_conversion),
        )
        .route("/config/reload", post(routes::config::reload_config))
        .route("/config/browse", get(routes::config::browse_path))
        // Conversions
        .route("/conversions", get(routes::conversions::list_conversions))
        .route(
            "/conversions/submit",
            post(routes::conversions::submit_conversion),
        )
        .route(
            "/conversions/{id}",
            get(routes::conversions::get_conversion).delete(routes::conversions::delete_conversion),
        )
        // Playback
        .route(
            "/playback/continue",
            get(routes::playback::continue_watching),
        )
        .route("/playback/{item_id}", get(routes::playback::get_playback))
        .route(
            "/playback/{item_id}/progress",
            post(routes::playback::update_progress),
        )
        .route(
            "/playback/{item_id}/played",
            post(routes::playback::mark_played),
        )
        .route(
            "/playback/{item_id}/unplayed",
            post(routes::playback::mark_unplayed),
        )
        .route(
            "/playback/{item_id}/user-data",
            get(routes::playback::get_user_data),
        )
        // Favorites
        .route("/favorites", get(routes::playback::list_favorites))
        .route(
            "/favorites/{item_id}",
            post(routes::playback::add_favorite).delete(routes::playback::remove_favorite),
        )
        // Streaming
        .route(
            "/stream/{media_file_id}/index.m3u8",
            get(routes::stream::hls_playlist),
        )
        .route(
            "/stream/{media_file_id}/direct",
            get(routes::stream::direct_stream),
        )
        .route(
            "/stream/{media_file_id}/{segment}",
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

    // Always apply auth middleware — it handles both enabled (validates
    // credentials) and disabled (injects anonymous UserId) modes.
    // Without this, Extension<UserId> extractors on playback/favorites
    // routes would fail with 500 when auth is disabled.
    let protected_routes =
        protected_routes.layer(middleware::from_fn_with_state(ctx.clone(), auth_middleware));

    // Combine auth and protected under /api.
    let api = auth_routes.merge(protected_routes);

    let mut app = Router::new()
        .route("/health", get(routes::health::health_check))
        .nest("/api", api)
        .merge(SwaggerUi::new("/api-docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
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
