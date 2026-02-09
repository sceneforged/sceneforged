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

use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;

use crate::context::AppContext;

/// Middleware that logs response bodies for Jellyfin endpoints (debug level).
async fn log_jellyfin_response(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();

    let response = next.run(req).await;
    let status = response.status();

    // Only log bodies for JSON responses (not streams/images).
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    if content_type.contains("json") {
        let (parts, body) = response.into_parts();
        let bytes = axum::body::to_bytes(body, 64 * 1024)
            .await
            .unwrap_or_default();

        let body_str = String::from_utf8_lossy(&bytes);
        tracing::debug!(
            %method, %uri, %status,
            body = %body_str,
            "jellyfin response"
        );

        Response::from_parts(parts, Body::from(bytes))
    } else {
        tracing::debug!(%method, %uri, %status, content_type, "jellyfin response (non-json)");
        response
    }
}

/// Fallback for unmatched Jellyfin-style paths.
///
/// Without this, unmatched paths fall through to the SvelteKit static file
/// handler which returns HTML with 200 — causing clients to choke on
/// unexpected content.
async fn jellyfin_fallback(req: Request) -> impl IntoResponse {
    let uri = req.uri().clone();
    let method = req.method().clone();
    tracing::warn!(%method, %uri, "unhandled Jellyfin API path");
    (
        StatusCode::NOT_FOUND,
        [("content-type", "application/json")],
        format!(r#"{{"error":"Not Found","path":"{}"}}"#, uri.path()),
    )
}

/// Build the Jellyfin-compatible router.
///
/// These paths are mounted at the root level (not under /api) to match
/// what Jellyfin clients expect.
pub fn jellyfin_router() -> Router<AppContext> {
    // Common Jellyfin path prefixes — catch-all so unhandled paths return
    // JSON 404 instead of the SvelteKit HTML fallback.
    let jellyfin_catchall = Router::new()
        .fallback(jellyfin_fallback);

    Router::new()
        // System
        .route("/System/Info/Public", get(system::system_info_public))
        .route("/System/Info", get(system::system_info))
        .route("/QuickConnect/Enabled", get(system::quick_connect_enabled))
        .route("/Branding/Configuration", get(system::branding_configuration))
        .route("/Library/VirtualFolders", get(items::virtual_folders))
        // Users
        .route("/Users/Public", get(users::public_users))
        .route(
            "/Users/AuthenticateByName",
            post(users::authenticate_by_name),
        )
        .route("/Users/Me", get(users::get_me))
        .route("/Users/{user_id}", get(users::get_user))
        // Display preferences (must exist or Infuse login flow stalls)
        .route("/DisplayPreferences/{id}", get(users::display_preferences))
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
        // Session capabilities (Infuse sends these immediately after auth)
        .route("/Sessions/Capabilities/Full", post(playstate::capabilities_full))
        .route("/Sessions/Capabilities", post(playstate::capabilities))
        .route("/Sessions/Playing/Ping", post(playstate::playing_ping))
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
        // User-scoped aliases (Infuse uses /Users/{user_id}/Views etc.)
        .route("/Users/{user_id}/Views", get(items::user_views))
        .route("/Users/{user_id}/Items", get(items::list_items))
        .route("/Users/{user_id}/Items/{id}", get(items::user_scoped_get_item))
        .route("/Users/{user_id}/GroupingOptions", get(items::grouping_options))
        // User-scoped home screen routes (Infuse Continue Watching / Recently Added)
        .route("/Users/{user_id}/Items/Resume", get(items::user_resume))
        .route("/Users/{user_id}/Items/Latest", get(items::user_latest))
        // Mark played/unplayed
        .route(
            "/Users/{user_id}/PlayedItems/{item_id}",
            post(playstate::mark_played).delete(playstate::mark_unplayed),
        )
        // Favorite toggle
        .route(
            "/Users/{user_id}/FavoriteItems/{item_id}",
            post(playstate::add_favorite).delete(playstate::remove_favorite),
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
        // Jellyfin subtitle delivery
        .route(
            "/Videos/{id}/{media_source_id}/Subtitles/{index}/0/Stream.vtt",
            get(streaming::jellyfin_subtitle),
        )
        // Catch-all for common Jellyfin prefixes — prevents SvelteKit HTML fallback.
        .nest("/System", jellyfin_catchall.clone())
        .nest("/Users", jellyfin_catchall.clone())
        .nest("/Items", jellyfin_catchall.clone())
        .nest("/Shows", jellyfin_catchall.clone())
        .nest("/Videos", jellyfin_catchall.clone())
        .nest("/Sessions", jellyfin_catchall.clone())
        .nest("/Search", jellyfin_catchall.clone())
        .nest("/DisplayPreferences", jellyfin_catchall.clone())
        .nest("/Branding", jellyfin_catchall.clone())
        .nest("/QuickConnect", jellyfin_catchall.clone())
        .nest("/Library", jellyfin_catchall.clone())
        .nest("/Notifications", jellyfin_catchall.clone())
        .nest("/Plugins", jellyfin_catchall)
        // Log response bodies for debugging client compatibility issues.
        .layer(middleware::from_fn(log_jellyfin_response))
}
