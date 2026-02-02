use crate::config::{ArrConfig, Config, ConversionConfig, JellyfinConfig, Rule};
use crate::conversion::ConversionManager;
use crate::state::AppState;
use crate::streaming::{self, start_cleanup_task, SessionManager};
use anyhow::{Context, Result};
use axum::{
    http::{header, Method, StatusCode},
    middleware,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use parking_lot::RwLock;
use sceneforged_db::pool::DbPool;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal;
use tower_http::{
    cors::{Any, CorsLayer},
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

pub mod auth;
pub mod openapi;
pub mod routes_admin;
pub mod routes_api;
pub mod routes_config;
pub mod routes_library;
pub mod routes_playback;
pub mod routes_sse;
pub mod routes_webhook;

/// Shared application context
#[derive(Clone)]
pub struct AppContext {
    pub state: Arc<AppState>,
    pub config: Arc<Config>,
    /// Path to config file (for persistence)
    pub config_path: Option<PathBuf>,
    /// Mutable rules (can be edited via API)
    pub rules: Arc<RwLock<Vec<Rule>>>,
    /// Mutable arr configs (can be edited via API)
    pub arrs: Arc<RwLock<Vec<ArrConfig>>>,
    /// Mutable jellyfin configs (can be edited via API)
    pub jellyfins: Arc<RwLock<Vec<JellyfinConfig>>>,
    /// Mutable conversion config (can be edited via API)
    pub conversion_config: Arc<RwLock<ConversionConfig>>,
    /// Database connection pool (optional for backwards compatibility)
    pub db_pool: Option<DbPool>,
    /// Session manager for tracking active streams
    pub session_manager: Option<Arc<SessionManager>>,
    /// Conversion manager for profile conversions
    pub conversion_manager: Option<Arc<ConversionManager>>,
}

/// Create the Axum router with all routes
pub fn create_router(ctx: AppContext, static_dir: Option<PathBuf>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

    let mut app = Router::new()
        // Health check
        .route("/health", get(health_check))
        // API routes (with optional auth)
        .nest("/api", api_routes(&ctx))
        // OpenAPI documentation (Swagger UI at /api/docs)
        .nest("/api", openapi::openapi_routes())
        // Webhook routes (with optional signature verification)
        .nest("/webhook", webhook_routes(&ctx));

    // Add streaming routes if database pool is available
    // These are nested under /api for consistency with other API routes
    if ctx.db_pool.is_some() {
        app = app
            .nest("/api/stream", streaming::hls_router())
            .nest("/api/direct", streaming::direct_router())
            .nest("/api/play", streaming::play_router());
        tracing::info!("Streaming routes enabled");
    }

    let mut app = app
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(ctx);

    // Serve static files if directory is provided
    // Uses SPA fallback: serves index.html for any route that doesn't match a file
    if let Some(dir) = static_dir {
        if dir.exists() {
            tracing::info!("Serving static files from {:?}", dir);
            let index_path = dir.join("index.html");
            app = app.fallback_service(
                ServeDir::new(&dir)
                    .append_index_html_on_directories(true)
                    .not_found_service(ServeFile::new(index_path)),
            );
        }
    }

    app
}

fn api_routes(ctx: &AppContext) -> Router<AppContext> {
    // Auth routes (always available, even when auth is disabled)
    let auth_routes = Router::new()
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/status", get(auth::auth_status));

    // Protected routes
    let protected_routes = routes_api::api_routes()
        .merge(routes_sse::sse_routes())
        .merge(routes_config::config_routes())
        .merge(routes_library::library_routes())
        .merge(routes_playback::playback_routes())
        .merge(routes_admin::admin_routes());

    // Apply auth middleware to protected routes only if enabled
    let protected_routes = if ctx.config.server.auth.enabled {
        protected_routes.layer(middleware::from_fn_with_state(
            ctx.clone(),
            auth::api_auth_middleware,
        ))
    } else {
        protected_routes
    };

    // Merge auth routes (no middleware) with protected routes
    auth_routes.merge(protected_routes)
}

fn webhook_routes(ctx: &AppContext) -> Router<AppContext> {
    routes_webhook::webhook_routes(ctx)
}

async fn health_check() -> impl IntoResponse {
    StatusCode::OK
}

/// Start the HTTP server
pub async fn start_server(config: Config, state: Arc<AppState>) -> Result<()> {
    start_server_with_options(config, state, None, None).await
}

/// Start the HTTP server with an optional config path for persistence
pub async fn start_server_with_config_path(
    config: Config,
    state: Arc<AppState>,
    config_path: Option<PathBuf>,
) -> Result<()> {
    start_server_with_options(config, state, config_path, None).await
}

/// Start the HTTP server with full options including database pool
pub async fn start_server_with_options(
    config: Config,
    state: Arc<AppState>,
    config_path: Option<PathBuf>,
    db_pool: Option<DbPool>,
) -> Result<()> {
    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port)
        .parse()
        .context("Invalid server address")?;

    // Initialize session manager if database is available
    let session_manager = db_pool.as_ref().map(|_| {
        let manager = SessionManager::default();
        // Start cleanup task
        start_cleanup_task(manager.clone(), 30);
        Arc::new(manager)
    });

    // Initialize conversion manager if database is available
    let conversion_manager = db_pool
        .as_ref()
        .map(|pool| Arc::new(ConversionManager::new(pool.clone())));

    let ctx = AppContext {
        state,
        rules: Arc::new(RwLock::new(config.rules.clone())),
        arrs: Arc::new(RwLock::new(config.arrs.clone())),
        jellyfins: Arc::new(RwLock::new(config.jellyfins.clone())),
        conversion_config: Arc::new(RwLock::new(config.conversion.clone())),
        config: Arc::new(config.clone()),
        config_path,
        db_pool,
        session_manager,
        conversion_manager,
    };

    let app = create_router(ctx, config.server.static_dir.clone());

    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Server shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        match signal::ctrl_c().await {
            Ok(()) => {}
            Err(e) => {
                tracing::error!("Failed to install Ctrl+C handler: {}", e);
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                tracing::error!("Failed to install SIGTERM handler: {}", e);
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received");
}
