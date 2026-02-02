use crate::config::JellyfinConfig;
use anyhow::Result;
use reqwest::Client;
use std::time::Duration;

const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);

pub struct JellyfinClient {
    client: Client,
    base_url: String,
    api_key: String,
    name: String,
}

impl JellyfinClient {
    pub fn new(config: &JellyfinConfig) -> Self {
        let client = Client::builder()
            .timeout(CONNECTION_TIMEOUT)
            .build()
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to build HTTP client: {}", e);
                Client::new()
            });

        Self {
            client,
            base_url: config.url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
            name: config.name.clone(),
        }
    }

    /// Trigger a full library refresh
    pub async fn refresh_library(&self) -> Result<()> {
        let url = format!("{}/Library/Refresh", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("X-Emby-Token", &self.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Jellyfin refresh failed ({}): {}", status, body);
        }

        Ok(())
    }

    /// Test connectivity to Jellyfin
    pub async fn test_connection(&self) -> Result<bool> {
        let url = format!("{}/System/Info", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("X-Emby-Token", &self.api_key)
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
