use crate::config::{ArrConfig, ArrType};
use anyhow::{Context, Result};
use reqwest::Client;
use serde::Serialize;
use std::time::Duration;

/// Connection timeout for Arr API requests
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);

/// Common trait for *arr API clients
#[async_trait::async_trait]
pub trait ArrClient: Send + Sync {
    /// Test the connection to the *arr instance
    async fn test_connection(&self) -> Result<bool>;

    /// Trigger a rescan for a specific item
    async fn rescan(&self, item_id: i64) -> Result<()>;

    /// Trigger a rename for a specific item
    async fn rename(&self, item_id: i64) -> Result<()>;
}

/// Create an appropriate client based on config
pub fn create_client(config: &ArrConfig) -> Box<dyn ArrClient> {
    match config.arr_type {
        ArrType::Radarr => Box::new(RadarrClient::new(config)),
        ArrType::Sonarr => Box::new(SonarrClient::new(config)),
    }
}

struct BaseArrClient {
    client: Client,
    base_url: String,
    api_key: String,
}

impl BaseArrClient {
    fn new(config: &ArrConfig) -> Self {
        let client = Client::builder()
            .timeout(CONNECTION_TIMEOUT)
            .build()
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to build HTTP client with timeout: {}", e);
                Client::new()
            });

        Self {
            client,
            base_url: config.url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/api/v3{}", self.base_url, path)
    }

    async fn get(&self, path: &str) -> Result<reqwest::Response> {
        self.client
            .get(self.url(path))
            .header("X-Api-Key", &self.api_key)
            .send()
            .await
            .context(format!("Failed to GET {}", path))
    }

    async fn post_command<T: Serialize>(&self, command: &T, context_msg: &str) -> Result<()> {
        let context_msg = context_msg.to_string();
        let response = self
            .client
            .post(self.url("/command"))
            .header("X-Api-Key", &self.api_key)
            .json(command)
            .send()
            .await
            .context(context_msg.clone())?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            anyhow::bail!("{}: {}", context_msg, error);
        }
        Ok(())
    }
}

pub struct RadarrClient(BaseArrClient);

impl RadarrClient {
    pub fn new(config: &ArrConfig) -> Self {
        Self(BaseArrClient::new(config))
    }
}

#[async_trait::async_trait]
impl ArrClient for RadarrClient {
    async fn test_connection(&self) -> Result<bool> {
        let response = self.0.get("/system/status").await?;
        Ok(response.status().is_success())
    }

    async fn rescan(&self, movie_id: i64) -> Result<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct RescanCommand {
            name: &'static str,
            movie_ids: Vec<i64>,
        }

        let command = RescanCommand {
            name: "RescanMovie",
            movie_ids: vec![movie_id],
        };

        self.0
            .post_command(&command, "Failed to trigger Radarr rescan")
            .await
    }

    async fn rename(&self, movie_id: i64) -> Result<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct RenameCommand {
            name: &'static str,
            movie_ids: Vec<i64>,
        }

        let command = RenameCommand {
            name: "RenameMovie",
            movie_ids: vec![movie_id],
        };

        self.0
            .post_command(&command, "Failed to trigger Radarr rename")
            .await
    }
}

pub struct SonarrClient(BaseArrClient);

impl SonarrClient {
    pub fn new(config: &ArrConfig) -> Self {
        Self(BaseArrClient::new(config))
    }
}

#[async_trait::async_trait]
impl ArrClient for SonarrClient {
    async fn test_connection(&self) -> Result<bool> {
        let response = self.0.get("/system/status").await?;
        Ok(response.status().is_success())
    }

    async fn rescan(&self, series_id: i64) -> Result<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct RescanCommand {
            name: &'static str,
            series_id: i64,
        }

        let command = RescanCommand {
            name: "RescanSeries",
            series_id,
        };

        self.0
            .post_command(&command, "Failed to trigger Sonarr rescan")
            .await
    }

    async fn rename(&self, series_id: i64) -> Result<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct RenameCommand {
            name: &'static str,
            series_id: i64,
        }

        let command = RenameCommand {
            name: "RenameSeries",
            series_id,
        };

        self.0
            .post_command(&command, "Failed to trigger Sonarr rename")
            .await
    }
}
