//! Media streaming module.
//!
//! Provides HLS and direct streaming for media files.
//!
//! # Profile Streaming
//!
//! - **Profile A**: Direct streaming via HTTP range requests for MKV, 4K HDR/DV content
//! - **Profile B**: HLS fMP4 streaming for MP4, ≤1080p SDR, H.264 content
//!
//! # Routes
//!
//! HLS routes:
//! - `GET /stream/{media_file_id}/master.m3u8` - Master playlist (adaptive)
//! - `GET /stream/{media_file_id}/playlist.m3u8` - Media playlist (segments)
//! - `GET /stream/{media_file_id}/init.mp4` - Init segment (ftyp + moov)
//! - `GET /stream/{media_file_id}/segment/{index}.m4s` - Media segment
//!
//! Direct routes:
//! - `GET /direct/{media_file_id}` - Direct file streaming with range support
//! - `GET /play/{item_id}` - Auto-select best file and stream

mod direct;
mod hls;
mod segment_cache;

pub use direct::{stream_file, stream_item};
pub use hls::{init_segment, master_playlist, media_playlist, media_segment};
pub use segment_cache::SegmentCache;

use axum::{routing::get, Router};

use crate::server::AppContext;

/// Create HLS streaming router.
pub fn hls_router() -> Router<AppContext> {
    Router::new()
        .route("/:media_file_id/master.m3u8", get(master_playlist))
        .route("/:media_file_id/playlist.m3u8", get(media_playlist))
        .route("/:media_file_id/init.mp4", get(init_segment))
        .route("/:media_file_id/segment/:segment_index", get(media_segment))
}

/// Create direct streaming router.
pub fn direct_router() -> Router<AppContext> {
    Router::new().route("/:media_file_id", get(stream_file))
}

/// Create item-based streaming router (auto-selects best file).
pub fn play_router() -> Router<AppContext> {
    Router::new().route("/:item_id", get(stream_item))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hls_router_creation() {
        let _router: Router<AppContext> = hls_router();
    }

    #[test]
    fn test_direct_router_creation() {
        let _router: Router<AppContext> = direct_router();
    }

    #[test]
    fn test_play_router_creation() {
        let _router: Router<AppContext> = play_router();
    }
}
