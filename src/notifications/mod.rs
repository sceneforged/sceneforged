pub mod jellyfin;

pub use jellyfin::JellyfinClient;

use crate::config::Config;
use std::path::Path;

/// Manages all notification targets (Jellyfin, etc.)
pub struct NotificationManager {
    jellyfin_clients: Vec<JellyfinClient>,
}

impl NotificationManager {
    pub fn new(config: &Config) -> Self {
        let jellyfin_clients = config
            .jellyfins
            .iter()
            .filter(|j| j.enabled)
            .map(JellyfinClient::new)
            .collect();

        Self { jellyfin_clients }
    }

    /// Notify all configured targets about a completed job.
    /// This method is fire-and-forget - errors are logged but not propagated.
    pub async fn notify_job_completed(&self, file_path: &Path) {
        for client in &self.jellyfin_clients {
            let client_name = client.name().to_string();
            let path_display = file_path.display().to_string();

            match client.refresh_library().await {
                Ok(()) => {
                    tracing::info!(
                        "Jellyfin '{}' library refresh triggered for: {}",
                        client_name,
                        path_display
                    );
                }
                Err(e) => {
                    tracing::warn!("Failed to notify Jellyfin '{}': {}", client_name, e);
                }
            }
        }
    }

    /// Check if there are any enabled notification targets
    pub fn has_targets(&self) -> bool {
        !self.jellyfin_clients.is_empty()
    }
}
