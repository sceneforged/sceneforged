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
use sf_server::context::{AppContext, ConfigStore};
use sf_server::router::build_router;

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

    /// Get a database connection from the pool.
    pub fn conn(&self) -> sf_db::pool::PooledConnection {
        sf_db::pool::get_conn(&self.db).expect("failed to get db connection")
    }
}
