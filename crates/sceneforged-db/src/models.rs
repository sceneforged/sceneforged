//! Internal Rust models matching the database schema.
//!
//! This module provides strongly-typed Rust structures that map to database tables.
//! All models use types from sf-common where appropriate.

use chrono::{DateTime, Utc};
use sceneforged_common::{
    CheckpointId, FileRole, ImageId, ImageType, ItemId, ItemKind, LibraryId, MediaFileId,
    MediaType, StreamType, UserId,
};
use serde::{Deserialize, Serialize};

/// User account model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct User {
    pub id: UserId,
    pub username: String,
    pub password_hash: String,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
}

/// Media library model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Library {
    pub id: LibraryId,
    pub name: String,
    pub media_type: MediaType,
    pub paths: Vec<String>,
    pub created_at: DateTime<Utc>,
}

/// Library item model (movie, series, season, episode, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Item {
    pub id: ItemId,
    pub library_id: LibraryId,
    pub parent_id: Option<ItemId>,
    pub item_kind: ItemKind,
    pub name: String,
    pub sort_name: Option<String>,
    pub original_title: Option<String>,
    pub file_path: Option<String>,
    pub container: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub resolution: Option<String>,
    pub runtime_ticks: Option<i64>,
    pub size_bytes: Option<i64>,
    pub overview: Option<String>,
    pub tagline: Option<String>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    pub studios: Vec<String>,
    pub people: Vec<Person>,
    pub community_rating: Option<f64>,
    pub critic_rating: Option<f64>,
    pub production_year: Option<i32>,
    pub premiere_date: Option<String>,
    pub end_date: Option<String>,
    pub official_rating: Option<String>,
    pub provider_ids: ProviderIds,
    pub scene_release_name: Option<String>,
    pub scene_group: Option<String>,
    pub index_number: Option<i32>,
    pub parent_index_number: Option<i32>,
    pub etag: Option<String>,
    pub date_created: DateTime<Utc>,
    pub date_modified: DateTime<Utc>,
    pub hdr_type: Option<String>,
    pub dolby_vision_profile: Option<String>,
}

/// Person metadata (actor, director, writer, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Person {
    pub name: String,
    pub role: Option<String>,
    pub person_type: String,
    pub image_url: Option<String>,
}

/// External provider IDs (TMDB, IMDB, TVDB, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ProviderIds {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmdb: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imdb: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tvdb: Option<String>,
}

/// Media file model (source, universal, or extra file for an item).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MediaFile {
    pub id: MediaFileId,
    pub item_id: ItemId,
    pub role: FileRole,
    pub file_path: String,
    pub file_size: i64,
    pub container: String,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration_ticks: Option<i64>,
    pub bit_rate: Option<i64>,
    pub is_hdr: bool,
    pub serves_as_universal: bool,
    pub has_faststart: bool,
    pub keyframe_interval_secs: Option<f64>,
    pub created_at: DateTime<Utc>,
}

/// Conversion job status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConversionStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for ConversionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Queued => write!(f, "queued"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for ConversionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("Invalid conversion status: {}", s)),
        }
    }
}

/// Conversion job model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConversionJob {
    pub id: String,
    pub item_id: ItemId,
    pub source_file_id: MediaFileId,
    pub status: ConversionStatus,
    pub progress_pct: f64,
    pub output_path: Option<String>,
    pub error_message: Option<String>,
    pub hw_accel_used: Option<String>,
    pub encode_fps: Option<f64>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Media stream model (video, audio, subtitle).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MediaStream {
    pub id: String,
    pub media_file_id: MediaFileId,
    pub stream_type: StreamType,
    pub index_num: i32,
    pub codec: Option<String>,
    pub language: Option<String>,
    pub title: Option<String>,
    pub is_default: bool,
    pub is_forced: bool,
    pub is_external: bool,
    pub external_path: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub bit_rate: Option<i64>,
    pub frame_rate: Option<f64>,
    pub pixel_format: Option<String>,
    pub color_primaries: Option<String>,
    pub color_transfer: Option<String>,
    pub color_space: Option<String>,
    pub channels: Option<i32>,
    pub channel_layout: Option<String>,
    pub sample_rate: Option<i32>,
}

/// Image/artwork model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Image {
    pub id: ImageId,
    pub item_id: ItemId,
    pub image_type: ImageType,
    pub path: String,
    pub provider: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub tag: Option<String>,
}

/// User-specific item data (playback position, favorites, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserItemData {
    pub user_id: UserId,
    pub item_id: ItemId,
    pub playback_position_ticks: i64,
    pub play_count: i32,
    pub played: bool,
    pub is_favorite: bool,
    pub last_played_date: Option<DateTime<Utc>>,
}

/// Authentication token model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthToken {
    pub token: String,
    pub user_id: UserId,
    pub device_id: String,
    pub device_name: Option<String>,
    pub client_name: Option<String>,
    pub client_version: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
}

/// Sync change log entry for InfuseSync delta sync.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncChangeLog {
    pub id: i64,
    pub item_id: ItemId,
    pub change_type: String,
    pub changed_at: DateTime<Utc>,
}

/// Sync user data log entry for InfuseSync delta sync.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncUserDataLog {
    pub id: i64,
    pub user_id: UserId,
    pub item_id: ItemId,
    pub changed_at: DateTime<Utc>,
}

/// Sync checkpoint for tracking delta sync position per device.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncCheckpoint {
    pub id: CheckpointId,
    pub user_id: UserId,
    pub device_id: String,
    pub item_checkpoint: i64,
    pub user_data_checkpoint: i64,
    pub created_at: DateTime<Utc>,
    pub last_sync: DateTime<Utc>,
}

/// Media stream ID (UUID).
pub type MediaStreamId = String;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_serialization() {
        let user = User {
            id: UserId::new(),
            username: "testuser".to_string(),
            password_hash: "hash123".to_string(),
            is_admin: false,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&user).unwrap();
        let deserialized: User = serde_json::from_str(&json).unwrap();
        assert_eq!(user, deserialized);
    }

    #[test]
    fn test_library_serialization() {
        let library = Library {
            id: LibraryId::new(),
            name: "Movies".to_string(),
            media_type: MediaType::Movies,
            paths: vec!["/media/movies".to_string()],
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&library).unwrap();
        let deserialized: Library = serde_json::from_str(&json).unwrap();
        assert_eq!(library, deserialized);
    }

    #[test]
    fn test_person_default() {
        let person = Person::default();
        assert_eq!(person.name, "");
        assert_eq!(person.role, None);
        assert_eq!(person.person_type, "");
        assert_eq!(person.image_url, None);
    }

    #[test]
    fn test_provider_ids_serialization() {
        let ids = ProviderIds {
            tmdb: Some("12345".to_string()),
            imdb: Some("tt123456".to_string()),
            tvdb: None,
        };

        let json = serde_json::to_string(&ids).unwrap();
        let deserialized: ProviderIds = serde_json::from_str(&json).unwrap();
        assert_eq!(ids, deserialized);
    }

    #[test]
    fn test_provider_ids_skip_none() {
        let ids = ProviderIds {
            tmdb: Some("12345".to_string()),
            imdb: None,
            tvdb: None,
        };

        let json = serde_json::to_string(&ids).unwrap();
        // Should not include imdb or tvdb fields
        assert!(!json.contains("imdb"));
        assert!(!json.contains("tvdb"));
    }

    #[test]
    fn test_item_serialization() {
        let item = Item {
            id: ItemId::new(),
            library_id: LibraryId::new(),
            parent_id: None,
            item_kind: ItemKind::Movie,
            name: "Test Movie".to_string(),
            sort_name: None,
            original_title: None,
            file_path: Some("/media/movie.mkv".to_string()),
            container: Some("mkv".to_string()),
            video_codec: Some("hevc".to_string()),
            audio_codec: Some("aac".to_string()),
            resolution: Some("1920x1080".to_string()),
            runtime_ticks: Some(72000000000),
            size_bytes: Some(1024 * 1024 * 1024),
            overview: Some("A test movie".to_string()),
            tagline: None,
            genres: vec!["Action".to_string(), "Drama".to_string()],
            tags: vec![],
            studios: vec![],
            people: vec![],
            community_rating: Some(7.5),
            critic_rating: None,
            production_year: Some(2023),
            premiere_date: Some("2023-01-01".to_string()),
            end_date: None,
            official_rating: Some("PG-13".to_string()),
            provider_ids: ProviderIds::default(),
            scene_release_name: None,
            scene_group: None,
            index_number: None,
            parent_index_number: None,
            etag: None,
            date_created: Utc::now(),
            date_modified: Utc::now(),
            hdr_type: Some("hdr10".to_string()),
            dolby_vision_profile: None,
        };

        let json = serde_json::to_string(&item).unwrap();
        let deserialized: Item = serde_json::from_str(&json).unwrap();
        assert_eq!(item, deserialized);
    }

    #[test]
    fn test_media_stream_serialization() {
        let stream = MediaStream {
            id: uuid::Uuid::new_v4().to_string(),
            media_file_id: MediaFileId::new(),
            stream_type: StreamType::Video,
            index_num: 0,
            codec: Some("hevc".to_string()),
            language: Some("eng".to_string()),
            title: None,
            is_default: true,
            is_forced: false,
            is_external: false,
            external_path: None,
            width: Some(1920),
            height: Some(1080),
            bit_rate: Some(5000000),
            frame_rate: Some(23.976),
            pixel_format: Some("yuv420p10le".to_string()),
            color_primaries: Some("bt2020".to_string()),
            color_transfer: Some("smpte2084".to_string()),
            color_space: Some("bt2020nc".to_string()),
            channels: None,
            channel_layout: None,
            sample_rate: None,
        };

        let json = serde_json::to_string(&stream).unwrap();
        let deserialized: MediaStream = serde_json::from_str(&json).unwrap();
        assert_eq!(stream, deserialized);
    }

    #[test]
    fn test_media_file_serialization() {
        let file = MediaFile {
            id: MediaFileId::new(),
            item_id: ItemId::new(),
            role: FileRole::Source,
            file_path: "/media/movie.mkv".to_string(),
            file_size: 1024 * 1024 * 1024,
            container: "mkv".to_string(),
            video_codec: Some("hevc".to_string()),
            audio_codec: Some("aac".to_string()),
            width: Some(1920),
            height: Some(1080),
            duration_ticks: Some(72000000000),
            bit_rate: Some(5000000),
            is_hdr: true,
            serves_as_universal: false,
            has_faststart: false,
            keyframe_interval_secs: None,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&file).unwrap();
        let deserialized: MediaFile = serde_json::from_str(&json).unwrap();
        assert_eq!(file, deserialized);
    }

    #[test]
    fn test_conversion_status() {
        assert_eq!(ConversionStatus::Queued.to_string(), "queued");
        assert_eq!(ConversionStatus::Running.to_string(), "running");
        assert_eq!(ConversionStatus::Completed.to_string(), "completed");
        assert_eq!(ConversionStatus::Failed.to_string(), "failed");
        assert_eq!(ConversionStatus::Cancelled.to_string(), "cancelled");

        assert_eq!(
            "queued".parse::<ConversionStatus>().unwrap(),
            ConversionStatus::Queued
        );
        assert_eq!(
            "running".parse::<ConversionStatus>().unwrap(),
            ConversionStatus::Running
        );
    }

    #[test]
    fn test_conversion_job_serialization() {
        let job = ConversionJob {
            id: uuid::Uuid::new_v4().to_string(),
            item_id: ItemId::new(),
            source_file_id: MediaFileId::new(),
            status: ConversionStatus::Queued,
            progress_pct: 0.0,
            output_path: None,
            error_message: None,
            hw_accel_used: None,
            encode_fps: None,
            started_at: None,
            completed_at: None,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&job).unwrap();
        let deserialized: ConversionJob = serde_json::from_str(&json).unwrap();
        assert_eq!(job, deserialized);
    }

    #[test]
    fn test_user_item_data_serialization() {
        let data = UserItemData {
            user_id: UserId::new(),
            item_id: ItemId::new(),
            playback_position_ticks: 50000000,
            play_count: 1,
            played: true,
            is_favorite: false,
            last_played_date: Some(Utc::now()),
        };

        let json = serde_json::to_string(&data).unwrap();
        let deserialized: UserItemData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
