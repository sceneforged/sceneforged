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
use sf_core::{InvitationId, ItemId, LibraryId, MediaFileId, UserId};
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
            active_scans: Arc::new(DashMap::new()),
            sendfile_sndbuf: Arc::new(std::sync::atomic::AtomicU32::new(128 * 1024)),
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
                    if let Ok(n) = stream.peek(&mut peek_buf).await {
                        if let Some(route) = sendfile::classify_peek(&peek_buf[..n]) {
                            let std_stream = match stream.into_std() {
                                Ok(s) => s,
                                Err(_) => return,
                            };
                            tokio::task::spawn_blocking(move || {
                                let _ = sendfile::handle_sendfile(std_stream, &ctx, route);
                            })
                            .await
                            .ok();
                            return;
                        }
                    }
                    let io = TokioIo::new(stream);
                    let hyper_service = TowerToHyperService::new(app.into_service());
                    let _ = hyper::server::conn::http1::Builder::new()
                        .serve_connection(io, hyper_service)
                        .with_upgrades()
                        .await;
                });
            }
        });

        (harness, addr)
    }

    /// Get a database connection from the pool.
    pub fn conn(&self) -> sf_db::pool::PooledConnection {
        sf_db::pool::get_conn(&self.db).expect("failed to get db connection")
    }

    // -----------------------------------------------------------------------
    // Test data helpers
    // -----------------------------------------------------------------------

    /// Create a test library and return its ID as both typed and string.
    pub fn create_library(&self) -> (LibraryId, String) {
        let conn = self.conn();
        let lib = sf_db::queries::libraries::create_library(
            &conn,
            "Test Movies",
            "movies",
            &["/media/movies".into()],
            &serde_json::json!({}),
        )
        .expect("failed to create test library");
        let id_str = lib.id.to_string();
        (lib.id, id_str)
    }

    /// Create a named library with a given media_type.
    pub fn create_library_named(&self, name: &str, media_type: &str) -> (LibraryId, String) {
        let conn = self.conn();
        let lib = sf_db::queries::libraries::create_library(
            &conn,
            name,
            media_type,
            &[],
            &serde_json::json!({}),
        )
        .expect("failed to create test library");
        let id_str = lib.id.to_string();
        (lib.id, id_str)
    }

    /// Create a test item + source media file, returning (item_id, item_id_string).
    pub fn create_item(&self, library_id: LibraryId) -> (ItemId, String) {
        let conn = self.conn();
        let item = sf_db::queries::items::create_item(
            &conn,
            library_id,
            "movie",
            "Test Movie",
            None,
            Some(2024),
            Some("A test movie"),
            Some(120),
            Some(7.5),
            None,
            None,
            None,
            None,
        )
        .expect("failed to create test item");

        // Create a source media file for the item.
        sf_db::queries::media_files::create_media_file(
            &conn,
            item.id,
            "/media/movies/test.mkv",
            "test.mkv",
            1024 * 1024 * 500,
            Some("mkv"),
            Some("hevc"),
            Some("aac"),
            Some(1920),
            Some(1080),
            None,
            false,
            None,
            "source",
            "C",
            Some(7200.0),
        )
        .expect("failed to create test media file");

        let id_str = item.id.to_string();
        (item.id, id_str)
    }

    /// Create a named item with specified kind and a source media file.
    /// Returns (item_id, media_file_id, item_id_string, mf_id_string).
    pub fn create_item_with_media(
        &self,
        library_id: LibraryId,
        name: &str,
        kind: &str,
    ) -> (ItemId, MediaFileId, String, String) {
        let conn = self.conn();
        let item = sf_db::queries::items::create_item(
            &conn,
            library_id,
            kind,
            name,
            None,
            Some(2024),
            None,
            Some(120),
            None,
            None,
            None,
            None,
            None,
        )
        .expect("failed to create test item");

        let mf = sf_db::queries::media_files::create_media_file(
            &conn,
            item.id,
            &format!("/media/{name}.mkv"),
            &format!("{name}.mkv"),
            1024 * 1024 * 500,
            Some("mkv"),
            Some("hevc"),
            Some("aac"),
            Some(1920),
            Some(1080),
            None,
            false,
            None,
            "source",
            "C",
            Some(7200.0),
        )
        .expect("failed to create test media file");

        (item.id, mf.id, item.id.to_string(), mf.id.to_string())
    }

    /// Create a bcrypt-hashed user and return (user_id, user_id_string).
    pub fn create_user(&self, username: &str, password: &str) -> (UserId, String) {
        let hash = bcrypt::hash(password, 4).expect("bcrypt hash failed"); // cost=4 for speed
        let conn = self.conn();
        let user = sf_db::queries::users::create_user(&conn, username, &hash, "user")
            .expect("failed to create test user");
        let id_str = user.id.to_string();
        (user.id, id_str)
    }

    /// Create an auth token in the DB and return the raw token string.
    pub fn auth_token(&self, user_id: UserId) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        let expires = chrono::Utc::now() + chrono::Duration::days(30);
        let conn = self.conn();
        sf_db::queries::auth::create_token(&conn, user_id, &token, &expires.to_rfc3339())
            .expect("failed to create auth token");
        token
    }

    /// Create a bcrypt-hashed admin user and return (user_id, user_id_string).
    pub fn create_admin_user(&self, username: &str, password: &str) -> (UserId, String) {
        let hash = bcrypt::hash(password, 4).expect("bcrypt hash failed");
        let conn = self.conn();
        let user = sf_db::queries::users::create_user(&conn, username, &hash, "admin")
            .expect("failed to create admin user");
        let id_str = user.id.to_string();
        (user.id, id_str)
    }

    /// Create an invitation via DB and return (invitation_id, code).
    pub fn create_invitation(
        &self,
        role: &str,
        expires_in_days: i64,
        creator: UserId,
    ) -> (InvitationId, String) {
        let expires = (chrono::Utc::now() + chrono::Duration::days(expires_in_days)).to_rfc3339();
        let conn = self.conn();
        let inv =
            sf_db::queries::invitations::create_invitation(&conn, role, creator, &expires)
                .expect("failed to create invitation");
        (inv.id, inv.code)
    }

    /// Create an item + media file record pointing to a real file on disk.
    /// Returns (item_id, media_file_id, item_id_string, mf_id_string).
    pub fn create_item_with_real_media(
        &self,
        library_id: LibraryId,
        name: &str,
        file_path: &str,
        container: &str,
        video_codec: &str,
        audio_codec: &str,
        width: i32,
        height: i32,
        profile: &str,
        duration_secs: f64,
    ) -> (ItemId, MediaFileId, String, String) {
        let conn = self.conn();
        let item = sf_db::queries::items::create_item(
            &conn,
            library_id,
            "movie",
            name,
            None,
            Some(2024),
            None,
            Some((duration_secs / 60.0) as i32),
            None,
            None,
            None,
            None,
            None,
        )
        .expect("failed to create test item");

        let file_size = std::fs::metadata(file_path)
            .map(|m| m.len() as i64)
            .unwrap_or(0);

        let mf = sf_db::queries::media_files::create_media_file(
            &conn,
            item.id,
            file_path,
            std::path::Path::new(file_path)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap(),
            file_size,
            Some(container),
            Some(video_codec),
            Some(audio_codec),
            Some(width),
            Some(height),
            None,
            false,
            None,
            "universal",
            profile,
            Some(duration_secs),
        )
        .expect("failed to create test media file");

        (item.id, mf.id, item.id.to_string(), mf.id.to_string())
    }

    /// Create a subtitle track for a media file.
    pub fn create_subtitle_track(
        &self,
        media_file_id: MediaFileId,
        index: i32,
        codec: &str,
        language: Option<&str>,
    ) {
        let conn = self.conn();
        sf_db::queries::subtitle_tracks::create_subtitle_track(
            &conn,
            media_file_id,
            index,
            codec,
            language,
            false,
            false,
        )
        .expect("failed to create subtitle track");
    }

    /// Create a series → season → episodes hierarchy.
    /// Returns (series_id, season_id, Vec<episode_ids>).
    pub fn create_series_hierarchy(
        &self,
        library_id: LibraryId,
        series_name: &str,
        num_episodes: usize,
    ) -> (ItemId, ItemId, Vec<ItemId>) {
        let conn = self.conn();
        let series = sf_db::queries::items::create_item(
            &conn,
            library_id,
            "series",
            series_name,
            None,
            Some(2024),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("create series");

        let season = sf_db::queries::items::create_item(
            &conn,
            library_id,
            "season",
            "Season 1",
            None,
            None,
            None,
            None,
            None,
            None,
            Some(series.id),
            Some(1),
            None,
        )
        .expect("create season");

        let mut episode_ids = Vec::new();
        for i in 1..=num_episodes {
            let ep = sf_db::queries::items::create_item(
                &conn,
                library_id,
                "episode",
                &format!("Episode {i}"),
                None,
                None,
                None,
                Some(45),
                None,
                None,
                Some(season.id),
                Some(1),
                Some(i as i32),
            )
            .expect("create episode");

            // Create a media file for each episode.
            sf_db::queries::media_files::create_media_file(
                &conn,
                ep.id,
                &format!("/media/{series_name}/S01E{i:02}.mkv"),
                &format!("S01E{i:02}.mkv"),
                1024 * 1024 * 200,
                Some("mkv"),
                Some("hevc"),
                Some("aac"),
                Some(1920),
                Some(1080),
                None,
                false,
                None,
                "source",
                "C",
                Some(2700.0),
            )
            .expect("create episode media file");

            episode_ids.push(ep.id);
        }

        (series.id, season.id, episode_ids)
    }
}
