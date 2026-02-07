//! Rust structs mapping to database tables.
//!
//! Each model implements `from_row` for constructing itself from a
//! `rusqlite::Row`.

use sf_core::{
    ConversionJobId, ImageId, ItemId, JobId, LibraryId, MediaFileId, SessionId, SubtitleTrackId,
    UserId,
};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Parse a UUID-based ID from a text column.
fn parse_id<T: From<Uuid>>(row: &rusqlite::Row, idx: usize) -> rusqlite::Result<T> {
    let s: String = row.get(idx)?;
    let uuid = Uuid::parse_str(&s)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, Box::new(e)))?;
    Ok(T::from(uuid))
}

fn parse_opt_id<T: From<Uuid>>(row: &rusqlite::Row, idx: usize) -> rusqlite::Result<Option<T>> {
    let s: Option<String> = row.get(idx)?;
    match s {
        Some(v) => {
            let uuid = Uuid::parse_str(&v)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, Box::new(e)))?;
            Ok(Some(T::from(uuid)))
        }
        None => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// User
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct User {
    pub id: UserId,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: String,
}

impl User {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: parse_id(row, 0)?,
            username: row.get(1)?,
            password_hash: row.get(2)?,
            role: row.get(3)?,
            created_at: row.get(4)?,
        })
    }
}

// ---------------------------------------------------------------------------
// AuthToken
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AuthToken {
    pub id: SessionId,
    pub user_id: UserId,
    pub token: String,
    pub expires_at: String,
}

impl AuthToken {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: parse_id(row, 0)?,
            user_id: parse_id(row, 1)?,
            token: row.get(2)?,
            expires_at: row.get(3)?,
        })
    }
}

// ---------------------------------------------------------------------------
// Library
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Library {
    pub id: LibraryId,
    pub name: String,
    pub media_type: String,
    pub paths: Vec<String>,
    pub config: serde_json::Value,
    pub created_at: String,
}

impl Library {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        let paths_json: String = row.get(3)?;
        let config_json: String = row.get(4)?;
        Ok(Self {
            id: parse_id(row, 0)?,
            name: row.get(1)?,
            media_type: row.get(2)?,
            paths: serde_json::from_str(&paths_json).unwrap_or_default(),
            config: serde_json::from_str(&config_json).unwrap_or(serde_json::Value::Object(Default::default())),
            created_at: row.get(5)?,
        })
    }
}

// ---------------------------------------------------------------------------
// Item
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Item {
    pub id: ItemId,
    pub library_id: LibraryId,
    pub item_kind: String,
    pub name: String,
    pub sort_name: Option<String>,
    pub year: Option<i32>,
    pub overview: Option<String>,
    pub runtime_minutes: Option<i32>,
    pub community_rating: Option<f64>,
    pub provider_ids: String,
    pub parent_id: Option<ItemId>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
}

impl Item {
    /// Build from a row selected as:
    /// id, library_id, item_kind, name, sort_name, year, overview,
    /// runtime_minutes, community_rating, provider_ids, parent_id,
    /// season_number, episode_number, created_at, updated_at
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: parse_id(row, 0)?,
            library_id: parse_id(row, 1)?,
            item_kind: row.get(2)?,
            name: row.get(3)?,
            sort_name: row.get(4)?,
            year: row.get(5)?,
            overview: row.get(6)?,
            runtime_minutes: row.get(7)?,
            community_rating: row.get(8)?,
            provider_ids: row.get::<_, Option<String>>(9)?.unwrap_or_else(|| "{}".to_string()),
            parent_id: parse_opt_id(row, 10)?,
            season_number: row.get(11)?,
            episode_number: row.get(12)?,
            created_at: row.get(13)?,
            updated_at: row.get(14)?,
        })
    }
}

// ---------------------------------------------------------------------------
// MediaFile
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MediaFile {
    pub id: MediaFileId,
    pub item_id: ItemId,
    pub file_path: String,
    pub file_name: String,
    pub file_size: i64,
    pub container: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub resolution_width: Option<i32>,
    pub resolution_height: Option<i32>,
    pub hdr_format: Option<String>,
    pub has_dolby_vision: bool,
    pub dv_profile: Option<i32>,
    pub role: String,
    pub profile: String,
    pub duration_secs: Option<f64>,
    pub created_at: String,
}

impl MediaFile {
    /// Build from a row selected as:
    /// id, item_id, file_path, file_name, file_size, container,
    /// video_codec, audio_codec, resolution_width, resolution_height,
    /// hdr_format, has_dolby_vision, dv_profile, role, profile,
    /// duration_secs, created_at
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: parse_id(row, 0)?,
            item_id: parse_id(row, 1)?,
            file_path: row.get(2)?,
            file_name: row.get(3)?,
            file_size: row.get(4)?,
            container: row.get(5)?,
            video_codec: row.get(6)?,
            audio_codec: row.get(7)?,
            resolution_width: row.get(8)?,
            resolution_height: row.get(9)?,
            hdr_format: row.get(10)?,
            has_dolby_vision: row.get::<_, i32>(11).unwrap_or(0) != 0,
            dv_profile: row.get(12)?,
            role: row.get(13)?,
            profile: row.get(14)?,
            duration_secs: row.get(15)?,
            created_at: row.get(16)?,
        })
    }
}

// ---------------------------------------------------------------------------
// Image
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Image {
    pub id: ImageId,
    pub item_id: ItemId,
    pub image_type: String,
    pub path: String,
    pub provider: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

impl Image {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: parse_id(row, 0)?,
            item_id: parse_id(row, 1)?,
            image_type: row.get(2)?,
            path: row.get(3)?,
            provider: row.get(4)?,
            width: row.get(5)?,
            height: row.get(6)?,
        })
    }
}

// ---------------------------------------------------------------------------
// SubtitleTrack
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SubtitleTrack {
    pub id: SubtitleTrackId,
    pub media_file_id: MediaFileId,
    pub track_index: i32,
    pub codec: String,
    pub language: Option<String>,
    pub forced: bool,
    pub default_track: bool,
    pub created_at: String,
}

impl SubtitleTrack {
    /// Build from a row selected as:
    /// id, media_file_id, track_index, codec, language, forced, default_track, created_at
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: parse_id(row, 0)?,
            media_file_id: parse_id(row, 1)?,
            track_index: row.get(2)?,
            codec: row.get(3)?,
            language: row.get(4)?,
            forced: row.get::<_, i32>(5).unwrap_or(0) != 0,
            default_track: row.get::<_, i32>(6).unwrap_or(0) != 0,
            created_at: row.get(7)?,
        })
    }
}

// ---------------------------------------------------------------------------
// Job
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Job {
    pub id: JobId,
    pub file_path: String,
    pub file_name: String,
    pub status: String,
    pub rule_name: Option<String>,
    pub progress: f64,
    pub current_step: Option<String>,
    pub error: Option<String>,
    pub source: Option<String>,
    pub retry_count: i32,
    pub max_retries: i32,
    pub priority: i32,
    pub locked_by: Option<String>,
    pub locked_at: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub scheduled_for: Option<String>,
}

impl Job {
    /// Build from a row selected as all columns in table order.
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: parse_id(row, 0)?,
            file_path: row.get(1)?,
            file_name: row.get(2)?,
            status: row.get(3)?,
            rule_name: row.get(4)?,
            progress: row.get::<_, f64>(5).unwrap_or(0.0),
            current_step: row.get(6)?,
            error: row.get(7)?,
            source: row.get(8)?,
            retry_count: row.get::<_, i32>(9).unwrap_or(0),
            max_retries: row.get::<_, i32>(10).unwrap_or(3),
            priority: row.get::<_, i32>(11).unwrap_or(0),
            locked_by: row.get(12)?,
            locked_at: row.get(13)?,
            created_at: row.get(14)?,
            started_at: row.get(15)?,
            completed_at: row.get(16)?,
            scheduled_for: row.get(17)?,
        })
    }
}

// ---------------------------------------------------------------------------
// ConversionJob
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ConversionJob {
    pub id: ConversionJobId,
    pub item_id: ItemId,
    pub media_file_id: Option<MediaFileId>,
    pub status: String,
    pub progress_pct: f64,
    pub encode_fps: Option<f64>,
    pub eta_secs: Option<i64>,
    pub error: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub locked_by: Option<String>,
    pub locked_at: Option<String>,
    pub source_media_file_id: Option<MediaFileId>,
}

impl ConversionJob {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: parse_id(row, 0)?,
            item_id: parse_id(row, 1)?,
            media_file_id: parse_opt_id(row, 2)?,
            status: row.get(3)?,
            progress_pct: row.get::<_, f64>(4).unwrap_or(0.0),
            encode_fps: row.get(5)?,
            eta_secs: row.get(6)?,
            error: row.get(7)?,
            created_at: row.get(8)?,
            started_at: row.get(9)?,
            completed_at: row.get(10)?,
            locked_by: row.get(11)?,
            locked_at: row.get(12)?,
            source_media_file_id: parse_opt_id(row, 13)?,
        })
    }
}

// ---------------------------------------------------------------------------
// Favorite
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Favorite {
    pub user_id: UserId,
    pub item_id: ItemId,
    pub created_at: String,
}

impl Favorite {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            user_id: parse_id(row, 0)?,
            item_id: parse_id(row, 1)?,
            created_at: row.get(2)?,
        })
    }
}

// ---------------------------------------------------------------------------
// Playback
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Playback {
    pub user_id: UserId,
    pub item_id: ItemId,
    pub position_secs: f64,
    pub completed: bool,
    pub play_count: i32,
    pub last_played_at: String,
}

impl Playback {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            user_id: parse_id(row, 0)?,
            item_id: parse_id(row, 1)?,
            position_secs: row.get::<_, f64>(2).unwrap_or(0.0),
            completed: row.get::<_, i32>(3).unwrap_or(0) != 0,
            play_count: row.get::<_, i32>(4).unwrap_or(0),
            last_played_at: row.get(5)?,
        })
    }
}
