use serde::{Deserialize, Serialize};

/// Radarr webhook payload
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RadarrWebhook {
    pub event_type: String,
    pub movie: Option<RadarrMovie>,
    pub movie_file: Option<RadarrMovieFile>,
    pub remote_movie: Option<RadarrRemoteMovie>,
    pub release: Option<RadarrRelease>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RadarrMovie {
    pub id: i64,
    pub title: String,
    pub file_path: Option<String>,
    pub folder_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RadarrMovieFile {
    pub id: i64,
    pub relative_path: Option<String>,
    pub path: Option<String>,
    pub quality: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RadarrRemoteMovie {
    pub title: Option<String>,
    pub year: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RadarrRelease {
    pub quality: Option<String>,
    pub release_title: Option<String>,
}

/// Sonarr webhook payload
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SonarrWebhook {
    pub event_type: String,
    pub series: Option<SonarrSeries>,
    pub episodes: Option<Vec<SonarrEpisode>>,
    pub episode_file: Option<SonarrEpisodeFile>,
    pub release: Option<SonarrRelease>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SonarrSeries {
    pub id: i64,
    pub title: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SonarrEpisode {
    pub id: i64,
    pub episode_number: i32,
    pub season_number: i32,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SonarrEpisodeFile {
    pub id: i64,
    pub relative_path: Option<String>,
    pub path: Option<String>,
    pub quality: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SonarrRelease {
    pub quality: Option<String>,
    pub release_title: Option<String>,
}

/// Generic webhook event that we extract from either Radarr or Sonarr
#[derive(Debug, Clone, Serialize)]
pub struct WebhookEvent {
    pub arr_name: String,
    pub event_type: String,
    pub file_path: Option<String>,
    pub title: String,
    /// Movie ID (Radarr) or Series ID (Sonarr) for callbacks
    pub item_id: Option<i64>,
}

impl RadarrWebhook {
    pub fn to_event(self, arr_name: &str) -> WebhookEvent {
        let file_path = self
            .movie_file
            .as_ref()
            .and_then(|f| f.path.clone())
            .or_else(|| self.movie.as_ref().and_then(|m| m.file_path.clone()));

        let item_id = self.movie.as_ref().map(|m| m.id);

        let title = self
            .movie
            .map(|m| m.title)
            .unwrap_or_else(|| "Unknown".to_string());

        WebhookEvent {
            arr_name: arr_name.to_string(),
            event_type: self.event_type,
            file_path,
            title,
            item_id,
        }
    }
}

impl SonarrWebhook {
    pub fn to_event(self, arr_name: &str) -> WebhookEvent {
        let file_path = self.episode_file.as_ref().and_then(|f| f.path.clone());

        let item_id = self.series.as_ref().map(|s| s.id);

        let title = self
            .series
            .map(|s| s.title)
            .unwrap_or_else(|| "Unknown".to_string());

        WebhookEvent {
            arr_name: arr_name.to_string(),
            event_type: self.event_type,
            file_path,
            title,
            item_id,
        }
    }
}
