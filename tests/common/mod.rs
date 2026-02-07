//! Shared test harness for integration tests.
//!
//! Provides [`TestHarness`] which creates an in-memory DB, default config,
//! EventBus, and full [`AppContext`]. The [`with_server`] constructor starts
//! Axum on a random port for HTTP-level testing.

use std::net::SocketAddr;
use std::sync::Arc;

use dashmap::DashMap;

use sf_av::ToolRegistry;
use sf_core::config::Config;
use sf_core::events::EventBus;
use sf_db::pool::{init_memory_pool, DbPool};
use sf_probe::{CompositeProber, Prober, RustProber};
use hyper_util::rt::TokioIo;
use hyper_util::service::TowerToHyperService;

use sf_server::context::{AppContext, ConfigStore};
use sf_server::router::build_router;
use sf_server::sendfile;

/// Test harness wrapping a fully-constructed [`AppContext`] backed by an
/// in-memory database.
pub struct TestHarness {
    pub ctx: AppContext,
    pub db: DbPool,
}

impl TestHarness {
    /// Create a new harness with default configuration and in-memory DB.
    pub fn new() -> Self {
        Self::with_config(Config::default())
    }

    /// Create a new harness with a custom configuration and in-memory DB.
    pub fn with_config(config: Config) -> Self {
        let db = init_memory_pool().expect("failed to create in-memory pool");
        let tools = Arc::new(ToolRegistry::discover(&config.tools));
        let prober: Arc<dyn Prober> =
            Arc::new(CompositeProber::new(vec![Box::new(RustProber::new())]));
        let config_store = Arc::new(ConfigStore::new(&config, None));
        let event_bus = Arc::new(EventBus::default());

        let ctx = AppContext {
            db: db.clone(),
            config: Arc::new(config),
            config_store,
            event_bus,
            prober,
            tools,
            hls_cache: Arc::new(DashMap::new()),
            hls_loading: Arc::new(DashMap::new()),
            active_conversions: Arc::new(DashMap::new()),
        };

        Self { ctx, db }
    }

    /// Start an Axum server on a random port and return the harness together
    /// with the bound socket address.
    pub async fn with_server() -> (Self, SocketAddr) {
        let harness = Self::new();
        let app = build_router(harness.ctx.clone(), None);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind random port");
        let addr = listener.local_addr().expect("failed to get local addr");

        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        (harness, addr)
    }

    /// Start an Axum server with custom config on a random port.
    pub async fn with_server_config(config: Config) -> (Self, SocketAddr) {
        let harness = Self::with_config(config);
        let app = build_router(harness.ctx.clone(), None);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind random port");
        let addr = listener.local_addr().expect("failed to get local addr");

        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        (harness, addr)
    }

    /// Start a server with the custom accept loop (peek + sendfile routing)
    /// on a random port. This uses the same connection dispatch logic as the
    /// real server: segment requests go through sendfile, everything else
    /// through hyper/Axum.
    pub async fn with_sendfile_server() -> (Self, SocketAddr) {
        Self::with_sendfile_server_config(Config::default()).await
    }

    /// Start a sendfile-routed server with custom config on a random port.
    pub async fn with_sendfile_server_config(config: Config) -> (Self, SocketAddr) {
        let harness = Self::with_config(config);
        let ctx = harness.ctx.clone();
        let app = build_router(ctx.clone(), None);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind random port");
        let addr = listener.local_addr().expect("failed to get local addr");

        tokio::spawn(async move {
            loop {
                let (stream, _) = match listener.accept().await {
                    Ok(conn) => conn,
                    Err(_) => break,
                };
                let ctx = ctx.clone();
                let app = app.clone();
                tokio::spawn(async move {
                    let mut peek_buf = [0u8; 256];
                    match stream.peek(&mut peek_buf).await {
                        Ok(n) if sendfile::is_segment_request(&peek_buf[..n]) => {
                            let std_stream = match stream.into_std() {
                                Ok(s) => s,
                                Err(_) => return,
                            };
                            tokio::task::spawn_blocking(move || {
                                let _ = sendfile::handle_sendfile_segment(std_stream, &ctx);
                            })
                            .await
                            .ok();
                        }
                        _ => {
                            let io = TokioIo::new(stream);
                            let hyper_service = TowerToHyperService::new(app.into_service());
                            let _ = hyper::server::conn::http1::Builder::new()
                                .serve_connection(io, hyper_service)
                                .with_upgrades()
                                .await;
                        }
                    }
                });
            }
        });

        (harness, addr)
    }

    /// Get a database connection from the pool.
    pub fn conn(&self) -> sf_db::pool::PooledConnection {
        sf_db::pool::get_conn(&self.db).expect("failed to get db connection")
    }
}
