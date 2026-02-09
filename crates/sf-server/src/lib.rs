//! sf-server: HTTP API server, background job processor, and file watcher.
//!
//! This crate ties together all other sf-* crates into a running server
//! application. It provides:
//!
//! - Axum-based HTTP API with authentication, rate limiting, and SSE
//! - Background job processor that dequeues work and runs pipelines
//! - File system watcher that auto-queues jobs for new media files
//! - Graceful shutdown via signal handling

pub mod context;
pub mod conversion_processor;
pub mod error;
pub mod hls_prep;
pub mod middleware;
pub mod notifications;
pub mod processor;
pub mod router;
pub mod routes;
pub mod scanner;
pub mod sendfile;
pub mod tmdb;
pub mod watcher;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use dashmap::DashMap;
use hyper_util::rt::TokioIo;
use hyper_util::service::TowerToHyperService;

use sf_core::config::Config;
use sf_core::events::EventBus;
use sf_probe::{CompositeProber, RustProber};
use tokio_util::sync::CancellationToken;

use crate::context::{AppContext, ConfigStore};

/// Start the sceneforged server.
///
/// This is the main entry point. It initializes the database, constructs the
/// [`AppContext`], and spawns the HTTP server, job processor, and file watcher.
/// Returns when a shutdown signal is received or the cancellation token is
/// triggered.
pub async fn start(config: Config, config_path: Option<PathBuf>) -> sf_core::Result<()> {
    // Validate configuration.
    for warning in config.validate() {
        tracing::warn!("Config warning: {warning}");
    }

    // Initialize database.
    let db_path = &config.server.db_path;
    let existed = db_path.exists();
    if let Some(parent) = db_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| {
                sf_core::Error::Io { source: e }
            })?;
            tracing::info!("Created database directory {}", parent.display());
        }
    }
    let db_str = db_path.to_string_lossy();
    let db = sf_db::pool::init_pool(&db_str)?;
    if existed {
        tracing::info!("Database opened (existing) at {db_str}");
    } else {
        tracing::info!("Database created (new) at {db_str}");
    }

    // Discover external tools.
    let tools = Arc::new(sf_av::ToolRegistry::discover(&config.tools));
    for info in tools.check_all() {
        if info.available {
            tracing::info!(
                "Tool found: {} ({})",
                info.name,
                info.version.as_deref().unwrap_or("unknown version")
            );
        } else {
            tracing::debug!("Tool not found: {}", info.name);
        }
    }

    // Build prober.
    let prober: Arc<dyn sf_probe::Prober> = Arc::new(
        CompositeProber::new(vec![Box::new(RustProber::new())]),
    );

    // Build config store.
    let config_store = Arc::new(ConfigStore::new(&config, config_path.clone()));

    // Build event bus.
    let event_bus = Arc::new(EventBus::default());

    let hls_cache = Arc::new(DashMap::new());
    let hls_loading = Arc::new(DashMap::new());
    let active_conversions = Arc::new(DashMap::new());
    let active_scans = Arc::new(DashMap::new());

    let ctx = AppContext {
        db,
        config: Arc::new(config.clone()),
        config_store,
        event_bus,
        prober,
        tools,
        hls_cache,
        hls_loading,
        active_conversions,
        active_scans,
    };

    // Cancellation token for graceful shutdown.
    let cancel = CancellationToken::new();

    // Spawn background job processor.
    let processor_ctx = ctx.clone();
    let processor_cancel = cancel.clone();
    let processor_handle = tokio::spawn(async move {
        processor::run_processor(processor_ctx, processor_cancel).await;
    });

    // Spawn conversion processor.
    let conv_ctx = ctx.clone();
    let conv_cancel = cancel.clone();
    let conv_handle = tokio::spawn(async move {
        conversion_processor::run_conversion_processor(conv_ctx, conv_cancel).await;
    });

    // Spawn file watcher.
    let watcher_ctx = ctx.clone();
    let watcher_cancel = cancel.clone();
    let watcher_handle = tokio::spawn(async move {
        watcher::run_watcher(watcher_ctx, watcher_cancel).await;
    });

    // Build and start the HTTP server.
    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port)
        .parse()
        .map_err(|e| sf_core::Error::Internal(format!("Invalid server address: {e}")))?;

    let app = router::build_router(ctx.clone(), config.server.static_dir.clone());

    tracing::info!("Starting server on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| sf_core::Error::Internal(format!("Failed to bind to {addr}: {e}")))?;

    let cancel_for_shutdown = cancel.clone();

    // Custom TCP accept loop: peek at each connection to route segment
    // requests to the sendfile handler, everything else through hyper/Axum.
    run_accept_loop(listener, ctx, app, cancel_for_shutdown).await;

    // Signal all background tasks to stop.
    cancel.cancel();

    // Wait for background tasks to finish.
    let _ = tokio::join!(processor_handle, conv_handle, watcher_handle);

    tracing::info!("Server shutdown complete");
    Ok(())
}

/// Accept loop that peeks at each connection to route segment requests to
/// the sendfile handler and everything else through hyper/Axum.
async fn run_accept_loop(
    listener: tokio::net::TcpListener,
    ctx: AppContext,
    app: Router,
    cancel: CancellationToken,
) {
    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, _addr)) => {
                        let ctx = ctx.clone();
                        let app = app.clone();
                        tokio::spawn(handle_connection(stream, ctx, app));
                    }
                    Err(e) => {
                        tracing::debug!("Accept error: {e}");
                    }
                }
            }
            _ = shutdown_signal(cancel.clone()) => break,
        }
    }
}

/// Handle a single TCP connection: peek to see if it's a sendfile-eligible
/// request, then either serve it via sendfile or pass it through to hyper/Axum.
async fn handle_connection(stream: tokio::net::TcpStream, ctx: AppContext, app: Router) {
    let mut peek_buf = [0u8; 256];

    // Try to route to the zero-copy sendfile handler.
    if let Ok(n) = stream.peek(&mut peek_buf).await {
        if let Some(route) = sendfile::classify_peek(&peek_buf[..n]) {
            let std_stream = match stream.into_std() {
                Ok(s) => s,
                Err(e) => {
                    tracing::debug!("Failed to convert to std TcpStream: {e}");
                    return;
                }
            };
            // tokio uses non-blocking sockets; switch to blocking so
            // sendfile(2) waits instead of returning EAGAIN immediately.
            let _ = std_stream.set_nonblocking(false);
            let _ = std_stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
            let _ = std_stream.set_write_timeout(Some(std::time::Duration::from_secs(30)));
            tokio::task::spawn_blocking(move || {
                if let Err(e) = sendfile::handle_sendfile(std_stream, &ctx, route) {
                    // Broken pipe is expected when clients probe video streams
                    // (e.g. Infuse reads a few bytes then disconnects).
                    if e.kind() == std::io::ErrorKind::BrokenPipe {
                        tracing::trace!("Sendfile client disconnected: {e}");
                    } else {
                        tracing::debug!("Sendfile error: {e}");
                    }
                }
            })
            .await
            .ok();
            return;
        }
    }

    // Normal Axum/hyper path.
    let io = TokioIo::new(stream);
    let hyper_service = TowerToHyperService::new(app.into_service());
    if let Err(e) = hyper::server::conn::http1::Builder::new()
        .serve_connection(io, hyper_service)
        .with_upgrades()
        .await
    {
        tracing::debug!("Hyper connection error: {e}");
    }
}

/// Wait for a shutdown signal (SIGINT or SIGTERM).
async fn shutdown_signal(cancel: CancellationToken) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
        _ = cancel.cancelled() => {}
    }

    tracing::info!("Shutdown signal received");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_builds_context() {
        // Verify that all the types compose correctly (compile-time check).
        let _config = Config::default();
    }
}
