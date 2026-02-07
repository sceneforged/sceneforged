//! Jellyfin-compatible API endpoints.
//!
//! These endpoints implement a subset of the Jellyfin API that allows
//! third-party clients (Swiftfin, Infuse, Jellyfin web) to browse
//! libraries, stream media, and track playback.

pub mod dto;
pub mod items;
pub mod playstate;
pub mod streaming;
pub mod system;
pub mod users;

use axum::routing::{get, post};
use axum::Router;

use crate::context::AppContext;

/// Build the Jellyfin-compatible router.
///
/// These paths are mounted at the root level (not under /api) to match
/// what Jellyfin clients expect.
pub fn jellyfin_router() -> Router<AppContext> {
    Router::new()
        // System
        .route("/System/Info/Public", get(system::system_info_public))
        .route("/System/Info", get(system::system_info))
        // Users
        .route("/Users/Public", get(users::public_users))
        .route(
            "/Users/AuthenticateByName",
            post(users::authenticate_by_name),
        )
        .route("/Users/{user_id}", get(users::get_user))
        // Items / library browsing
        .route("/UserViews", get(items::user_views))
        .route("/Items", get(items::list_items))
        .route("/Items/{id}", get(items::get_item))
        .route(
            "/Shows/{id}/Seasons",
            get(items::show_seasons),
        )
        .route(
            "/Shows/{id}/Episodes",
            get(items::show_episodes),
        )
        .route("/Shows/NextUp", get(items::next_up))
        .route("/Search/Hints", get(items::search_hints))
        // Playback info
        .route(
            "/Items/{id}/PlaybackInfo",
            post(streaming::playback_info),
        )
        // Playstate reporting
        .route("/Sessions/Playing", post(playstate::playing))
        .route(
            "/Sessions/Playing/Progress",
            post(playstate::progress),
        )
        .route(
            "/Sessions/Playing/Stopped",
            post(playstate::stopped),
        )
        // Images
        .route(
            "/Items/{id}/Images/{image_type}",
            get(items::get_image),
        )
        .route(
            "/Items/{id}/Images/{image_type}/{index}",
            get(items::get_image),
        )
        // Download (reuses direct stream handler)
        .route("/Items/{id}/Download", get(streaming::video_stream))
        // Streaming
        .route(
            "/Videos/{id}/stream",
            get(streaming::video_stream),
        )
        .route(
            "/Videos/{id}/master.m3u8",
            get(streaming::master_playlist),
        )
}
