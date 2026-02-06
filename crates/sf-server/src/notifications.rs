//! Post-completion notification system.
//!
//! Fires non-blocking notifications to external services (Jellyfin, Radarr,
//! Sonarr) after jobs or conversions complete. All notifications are
//! fire-and-forget: errors are logged but never propagate to the caller.

use std::time::Duration;

use reqwest::Client;

use sf_core::config::{ArrConfig, JellyfinConfig};

/// HTTP timeout for notification requests.
const NOTIFICATION_TIMEOUT: Duration = Duration::from_secs(10);

/// Manages HTTP notifications to external services.
///
/// Holds a shared [`reqwest::Client`] so connection pools are reused across
/// calls. All public methods log outcomes but never return errors.
pub struct NotificationManager {
    client: Client,
}

impl NotificationManager {
    /// Create a new notification manager with a pre-configured HTTP client.
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(NOTIFICATION_TIMEOUT)
            .build()
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to build notification HTTP client: {e}");
                Client::new()
            });

        Self { client }
    }

    // -----------------------------------------------------------------------
    // Jellyfin
    // -----------------------------------------------------------------------

    /// Trigger a library refresh on a Jellyfin server.
    ///
    /// This is fire-and-forget: errors are logged, not returned.
    pub async fn notify_jellyfin_refresh(&self, config: &JellyfinConfig) {
        let url = format!(
            "{}/Library/Refresh",
            config.url.trim_end_matches('/')
        );

        tracing::info!(
            jellyfin = %config.name,
            "Triggering Jellyfin library refresh"
        );

        match self
            .client
            .post(&url)
            .header("X-Emby-Token", &config.api_key)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!(
                    jellyfin = %config.name,
                    "Jellyfin library refresh triggered successfully"
                );
            }
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                tracing::warn!(
                    jellyfin = %config.name,
                    status = %status,
                    body = %body,
                    "Jellyfin library refresh returned non-success status"
                );
            }
            Err(e) => {
                tracing::warn!(
                    jellyfin = %config.name,
                    error = %e,
                    "Failed to contact Jellyfin for library refresh"
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Arr (Radarr / Sonarr)
    // -----------------------------------------------------------------------

    /// Trigger a rescan command on a Radarr or Sonarr instance.
    ///
    /// The `file_path` is used for logging context only; the actual rescan
    /// command tells the *arr to re-scan its disk for changes.
    ///
    /// This is fire-and-forget: errors are logged, not returned.
    pub async fn notify_arr_rescan(&self, config: &ArrConfig, file_path: &str) {
        let base_url = config.url.trim_end_matches('/');
        let command_url = format!("{base_url}/api/v3/command");

        let arr_type = config.arr_type.to_lowercase();

        tracing::info!(
            arr = %config.name,
            arr_type = %arr_type,
            file = %file_path,
            "Triggering arr rescan"
        );

        let body: serde_json::Value = match arr_type.as_str() {
            "radarr" => {
                serde_json::json!({
                    "name": "RescanMovie"
                })
            }
            "sonarr" => {
                serde_json::json!({
                    "name": "RescanSeries"
                })
            }
            other => {
                tracing::warn!(
                    arr = %config.name,
                    arr_type = %other,
                    "Unknown arr type, skipping rescan"
                );
                return;
            }
        };

        match self
            .client
            .post(&command_url)
            .header("X-Api-Key", &config.api_key)
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!(
                    arr = %config.name,
                    "Arr rescan command sent successfully"
                );
            }
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                tracing::warn!(
                    arr = %config.name,
                    status = %status,
                    body = %body,
                    "Arr rescan command returned non-success status"
                );
            }
            Err(e) => {
                tracing::warn!(
                    arr = %config.name,
                    error = %e,
                    "Failed to contact arr for rescan"
                );
            }
        }
    }
}

/// Spawn non-blocking notifications for all enabled Jellyfin instances.
///
/// Each notification runs in its own `tokio::spawn` so the caller is never
/// blocked.
pub fn spawn_jellyfin_notifications(
    manager: &NotificationManager,
    jellyfins: Vec<JellyfinConfig>,
) {
    for jf in jellyfins {
        if !jf.enabled {
            continue;
        }
        // Clone what we need to move into the spawned task.
        let client = manager.client.clone();
        tokio::spawn(async move {
            let mgr = NotificationManager { client };
            mgr.notify_jellyfin_refresh(&jf).await;
        });
    }
}

/// Spawn non-blocking arr rescan notifications for all enabled arrs with
/// `auto_rescan` enabled.
///
/// Each notification runs in its own `tokio::spawn` so the caller is never
/// blocked.
pub fn spawn_arr_notifications(
    manager: &NotificationManager,
    arrs: Vec<ArrConfig>,
    file_path: String,
) {
    for arr in arrs {
        if !arr.enabled || !arr.auto_rescan {
            continue;
        }
        let client = manager.client.clone();
        let path = file_path.clone();
        tokio::spawn(async move {
            let mgr = NotificationManager { client };
            mgr.notify_arr_rescan(&arr, &path).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_manager_creates_successfully() {
        let mgr = NotificationManager::new();
        // Verify the client was built (no panic).
        drop(mgr);
    }

    #[test]
    fn spawn_jellyfin_skips_disabled() {
        // This test verifies the filter logic without making HTTP calls.
        let configs = vec![
            JellyfinConfig {
                name: "disabled-jf".into(),
                url: "http://localhost:8096".into(),
                api_key: "test".into(),
                enabled: false,
            },
        ];

        let mgr = NotificationManager::new();

        // Should not panic even with unreachable URLs because disabled configs
        // are skipped before any HTTP call is made.
        spawn_jellyfin_notifications(&mgr, configs);
    }

    #[test]
    fn spawn_arr_skips_disabled_and_no_auto_rescan() {
        let configs = vec![
            ArrConfig {
                name: "disabled-arr".into(),
                arr_type: "radarr".into(),
                url: "http://localhost:7878".into(),
                api_key: "test".into(),
                enabled: false,
                auto_rescan: true,
                auto_rename: false,
            },
            ArrConfig {
                name: "no-rescan".into(),
                arr_type: "sonarr".into(),
                url: "http://localhost:8989".into(),
                api_key: "test".into(),
                enabled: true,
                auto_rescan: false,
                auto_rename: false,
            },
        ];

        let mgr = NotificationManager::new();

        // Should not panic: both configs are skipped.
        spawn_arr_notifications(&mgr, configs, "/path/to/file.mp4".into());
    }
}
