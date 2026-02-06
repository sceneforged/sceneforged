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
pub mod middleware;
pub mod notifications;
pub mod processor;
pub mod router;
pub mod routes;
pub mod scanner;
pub mod watcher;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

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
    let db_path = "sceneforged.db";
    let db = sf_db::pool::init_pool(db_path)?;
    tracing::info!("Database initialized at {db_path}");

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

    let ctx = AppContext {
        db,
        config: Arc::new(config.clone()),
        config_store,
        event_bus,
        prober,
        tools,
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

    let app = router::build_router(ctx, config.server.static_dir.clone());

    tracing::info!("Starting server on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| sf_core::Error::Internal(format!("Failed to bind to {addr}: {e}")))?;

    let cancel_for_shutdown = cancel.clone();

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(cancel_for_shutdown))
        .await
        .map_err(|e| sf_core::Error::Internal(format!("Server error: {e}")))?;

    // Signal all background tasks to stop.
    cancel.cancel();

    // Wait for background tasks to finish.
    let _ = tokio::join!(processor_handle, conv_handle, watcher_handle);

    tracing::info!("Server shutdown complete");
    Ok(())
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
