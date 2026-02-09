//! Jellyfin system info endpoints.

use axum::Json;
use serde::Serialize;

/// Jellyfin-compatible version string.
///
/// Infuse and other Jellyfin clients parse the server version to decide
/// which API features are available. Versions below 10.9 are rejected
/// by Infuse.  We report 10.9.0 for maximum compatibility.
pub const JELLYFIN_COMPAT_VERSION: &str = "10.9.0";

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SystemInfo {
    pub server_name: String,
    pub version: String,
    pub id: String,
    pub operating_system: String,
    pub product_name: String,
    pub startup_wizard_completed: bool,
    pub local_address: String,
}

/// GET /System/Info/Public
pub async fn system_info_public() -> Json<SystemInfo> {
    Json(SystemInfo {
        server_name: "SceneForged".into(),
        version: JELLYFIN_COMPAT_VERSION.into(),
        id: "sceneforged-server".into(),
        operating_system: std::env::consts::OS.into(),
        product_name: "SceneForged".into(),
        startup_wizard_completed: true,
        local_address: String::new(),
    })
}

/// GET /System/Info (authenticated)
pub async fn system_info() -> Json<SystemInfo> {
    system_info_public().await
}
