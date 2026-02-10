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
use std::sync::atomic::{AtomicU32, Ordering};
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

    // Determine SO_SNDBUF from config override or storage-class detection.
    let sendfile_sndbuf = Arc::new(AtomicU32::new(resolve_sndbuf(&config, &db)));

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
        sendfile_sndbuf,
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

    // Check kernel socket buffer limits before accepting connections.
    check_sndbuf_limits(ctx.sendfile_sndbuf.load(Ordering::Relaxed));

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

// ---------------------------------------------------------------------------
// SO_SNDBUF: storage-class detection
// ---------------------------------------------------------------------------

/// Default SO_SNDBUF for network-attached storage (NFS, CIFS, etc.).
/// Limits pipeline depth to avoid iowait spikes from concurrent NFS reads.
const SNDBUF_NETWORK: u32 = 128 * 1024;

/// Default SO_SNDBUF for local storage (SSD, HDD, etc.).
/// No NFS round-trip cost, so a larger buffer improves TCP batching.
const SNDBUF_LOCAL: u32 = 512 * 1024;

/// Storage classification for SO_SNDBUF tuning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StorageClass {
    Network,
    Local,
}

/// Resolve the SO_SNDBUF value from config override or storage detection.
fn resolve_sndbuf(config: &Config, db: &sf_db::pool::DbPool) -> u32 {
    // 1. Explicit config override wins.
    if let Some(val) = config.server.sndbuf {
        tracing::info!(sndbuf = val, "SO_SNDBUF from config override");
        return val;
    }

    // 2. Auto-detect from library storage paths.
    let paths = match sf_db::pool::get_conn(db)
        .ok()
        .and_then(|conn| sf_db::queries::libraries::list_libraries(&conn).ok())
    {
        Some(libs) => libs
            .iter()
            .flat_map(|lib| lib.paths.clone())
            .collect::<Vec<_>>(),
        None => Vec::new(),
    };

    if paths.is_empty() {
        tracing::info!(
            sndbuf = SNDBUF_NETWORK,
            "SO_SNDBUF defaulting to network-safe (no libraries configured)"
        );
        return SNDBUF_NETWORK;
    }

    let storage = detect_storage_class(&paths);
    let sndbuf = match storage {
        StorageClass::Network => SNDBUF_NETWORK,
        StorageClass::Local => SNDBUF_LOCAL,
    };
    tracing::info!(sndbuf, storage = ?storage, "SO_SNDBUF auto-configured from storage detection");
    sndbuf
}

/// Classify storage by checking if ALL paths reside on known-fast local filesystems.
///
/// Returns `Local` only when every path is on a known-fast FS (ext4, xfs, etc.).
/// Defaults to `Network` (conservative) for unknown/FUSE/overlay filesystems,
/// since Docker bind-mounts can mask NFS backends as fuseblk or overlay.
fn detect_storage_class(paths: &[String]) -> StorageClass {
    for path in paths {
        if !is_fast_local(path) {
            tracing::debug!(path, "not on known-fast local storage, using conservative buffer");
            return StorageClass::Network;
        }
    }
    StorageClass::Local
}

/// Filesystem types known to be fast local storage with no network round-trip
/// cost. Only these get the larger SO_SNDBUF; everything else (FUSE, overlay,
/// NFS, CIFS, unknown) defaults to the conservative network-safe buffer.
///
/// This is deliberately conservative: Docker bind-mounts can mask the real
/// storage backend (e.g. NFS on the host appears as `fuseblk` or `overlay`
/// inside the container). Better to under-buffer than over-buffer.
#[cfg(any(target_os = "linux", test))]
const FAST_LOCAL_FS_TYPES: &[&str] = &[
    "ext4", "ext3", "ext2", "xfs", "btrfs", "zfs", "f2fs", "bcachefs",
];

/// Check if a path resides on known-fast local storage.
#[cfg(target_os = "linux")]
fn is_fast_local(path: &str) -> bool {
    let mounts = match std::fs::read_to_string("/proc/mounts") {
        Ok(m) => m,
        Err(_) => return false,
    };
    is_fast_local_inner(path, &mounts)
}

/// Testable inner implementation: match path against parsed mount entries.
#[cfg(any(target_os = "linux", test))]
fn is_fast_local_inner(path: &str, mounts: &str) -> bool {
    let mut best_mountpoint = "";
    let mut best_fstype = "";

    for line in mounts.lines() {
        // Format: device mountpoint fstype options dump pass
        let mut parts = line.split_whitespace();
        let _device = parts.next();
        let mountpoint = match parts.next() {
            Some(m) => m,
            None => continue,
        };
        let fstype = match parts.next() {
            Some(f) => f,
            None => continue,
        };

        // Longest-prefix match: the mount whose mountpoint is the longest
        // prefix of our path is the one our file lives on.
        if path.starts_with(mountpoint)
            && (mountpoint == "/"
                || path.len() == mountpoint.len()
                || path.as_bytes().get(mountpoint.len()) == Some(&b'/'))
            && mountpoint.len() > best_mountpoint.len()
        {
            best_mountpoint = mountpoint;
            best_fstype = fstype;
        }
    }

    FAST_LOCAL_FS_TYPES.contains(&best_fstype)
}

/// Check if a path resides on known-fast local storage (macOS).
#[cfg(target_os = "macos")]
fn is_fast_local(path: &str) -> bool {
    use std::ffi::CString;

    let c_path = match CString::new(path) {
        Ok(p) => p,
        Err(_) => return false,
    };

    unsafe {
        let mut stat: libc::statfs = std::mem::zeroed();
        if libc::statfs(c_path.as_ptr(), &mut stat) != 0 {
            return false;
        }
        let fstype = std::ffi::CStr::from_ptr(stat.f_fstypename.as_ptr())
            .to_str()
            .unwrap_or("");
        matches!(fstype, "apfs" | "hfs")
    }
}

/// Log startup diagnostics about kernel socket buffer limits.
fn check_sndbuf_limits(requested: u32) {
    #[cfg(target_os = "linux")]
    {
        if let Ok(wmem_max) = std::fs::read_to_string("/proc/sys/net/core/wmem_max") {
            let wmem_str = wmem_max.trim();
            if let Ok(val) = wmem_str.parse::<u32>() {
                if val / 2 < requested {
                    tracing::warn!(
                        wmem_max = wmem_str,
                        effective_max = val / 2,
                        requested,
                        "SO_SNDBUF will be capped by kernel. \
                         Fix: sysctl -w net.core.wmem_max={}",
                        requested * 2
                    );
                } else {
                    tracing::info!(wmem_max = wmem_str, requested, "SO_SNDBUF limit OK");
                }
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = requested;
}

/// Set `SO_SNDBUF` on a socket and return the actual granted value.
fn set_sndbuf(stream: &std::net::TcpStream, size: u32) -> u32 {
    use std::os::fd::AsRawFd;
    let fd = stream.as_raw_fd();
    let val = size as libc::c_int;

    unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_SNDBUF,
            &val as *const libc::c_int as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        );
    }

    let mut actual: libc::c_int = 0;
    let mut len = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
    unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_SNDBUF,
            &mut actual as *mut libc::c_int as *mut libc::c_void,
            &mut len,
        );
    }

    actual.max(0) as u32
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

            // Direct play streams can be paused for minutes; HLS segments are short.
            let write_timeout = match &route {
                sendfile::PeekRoute::Direct { .. }
                | sendfile::PeekRoute::JellyfinStream { .. } => {
                    std::time::Duration::from_secs(300)
                }
                sendfile::PeekRoute::Segment { .. } => std::time::Duration::from_secs(30),
            };
            let _ = std_stream.set_write_timeout(Some(write_timeout));

            // Set SO_SNDBUF based on detected storage class. Network-attached
            // storage uses smaller buffers to limit NFS pipeline depth; local
            // storage uses larger buffers for better TCP batching.
            let sndbuf = ctx.sendfile_sndbuf.load(Ordering::Relaxed);
            let actual = set_sndbuf(&std_stream, sndbuf);
            tracing::debug!(requested = sndbuf, actual, "SO_SNDBUF set");
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

    // -- Storage class detection --

    const SAMPLE_MOUNTS: &str = "\
sysfs /sys sysfs rw,nosuid,nodev,noexec,relatime 0 0
proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0
/dev/sda1 / ext4 rw,relatime 0 0
/dev/sdb1 /data xfs rw,relatime 0 0
nas:/vol/media /mnt/nfs nfs4 rw,relatime,vers=4.2 0 0
tmpfs /tmp tmpfs rw,nosuid,nodev 0 0
//nas/share /mnt/smb cifs rw,relatime 0 0
/dev/sda1 /media/movies fuseblk rw,relatime 0 0";

    #[test]
    fn ext4_is_fast_local() {
        assert!(is_fast_local_inner("/home/user/video.mp4", SAMPLE_MOUNTS));
    }

    #[test]
    fn xfs_is_fast_local() {
        assert!(is_fast_local_inner("/data/local/video.mp4", SAMPLE_MOUNTS));
    }

    #[test]
    fn nfs_is_not_fast_local() {
        assert!(!is_fast_local_inner("/mnt/nfs/movies/test.mkv", SAMPLE_MOUNTS));
    }

    #[test]
    fn cifs_is_not_fast_local() {
        assert!(!is_fast_local_inner("/mnt/smb/files/doc.txt", SAMPLE_MOUNTS));
    }

    #[test]
    fn fuseblk_is_not_fast_local() {
        // Docker bind-mounts from NFS host appear as fuseblk — must be conservative.
        assert!(!is_fast_local_inner("/media/movies/film.mkv", SAMPLE_MOUNTS));
    }

    #[test]
    fn longest_prefix_wins() {
        let mounts = "\
/dev/sda1 / ext4 rw 0 0
nas:/vol /mnt/data nfs4 rw 0 0
/dev/sdc1 /mnt/data/local ext4 rw 0 0";
        // NFS mount — not fast local.
        assert!(!is_fast_local_inner("/mnt/data/remote/file.mkv", mounts));
        // ext4 submount under the NFS path — is fast local.
        assert!(is_fast_local_inner("/mnt/data/local/file.mkv", mounts));
    }

    #[test]
    fn exact_mountpoint_match() {
        // /data should not match /data-backup.
        let mounts = "\
/dev/sda1 / ext4 rw 0 0
nas:/vol /data nfs4 rw 0 0";
        assert!(!is_fast_local_inner("/data/file.mkv", mounts));
        // /data-backup falls through to / (ext4).
        assert!(is_fast_local_inner("/data-backup/file.mkv", mounts));
    }

    #[test]
    fn empty_mounts_defaults_conservative() {
        assert!(!is_fast_local_inner("/any/path", ""));
    }

    #[test]
    fn mixed_storage_uses_conservative() {
        // If any path is not fast-local, the whole set is classified as Network.
        let mounts = "\
/dev/sda1 / ext4 rw 0 0
/dev/sdb1 /data xfs rw 0 0
/dev/sdc1 /media fuseblk rw 0 0";

        // /data is xfs (fast), /media is fuseblk (not fast).
        assert!(is_fast_local_inner("/data/movies/file.mkv", mounts));
        assert!(!is_fast_local_inner("/media/movies/file.mkv", mounts));
    }

    #[test]
    fn config_sndbuf_override() {
        let mut config = Config::default();
        config.server.sndbuf = Some(262144);
        assert_eq!(config.server.sndbuf, Some(262144));
    }
}
