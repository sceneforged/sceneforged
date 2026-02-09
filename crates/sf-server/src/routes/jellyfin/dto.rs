//! Jellyfin-compatible data transfer objects.

use std::collections::HashMap;
use serde::Serialize;
use sf_db::queries::playback::UserItemData;

/// Ticks per second (Jellyfin uses 100ns ticks).
pub const TICKS_PER_SECOND: i64 = 10_000_000;

/// The main item type returned by Jellyfin APIs.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct BaseItemDto {
    pub id: String,
    pub name: String,
    pub server_id: String,
    #[serde(rename = "Type")]
    pub item_type: String,
    pub is_folder: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub production_year: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_time_ticks: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub community_rating: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_number: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_index_number: Option<i32>,
    pub image_tags: HashMap<String, String>,
    pub backdrop_image_tags: Vec<String>,
    pub user_data: UserDataDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_sources: Option<Vec<MediaSourceDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_streams: Option<Vec<MediaStreamDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    pub location_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recursive_item_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub provider_ids: HashMap<String, String>,
    pub genres: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserDataDto {
    pub played: bool,
    pub playback_position_ticks: i64,
    pub play_count: i32,
    pub is_favorite: bool,
    pub key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct MediaSourceDto {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_time_ticks: Option<i64>,
    pub supports_direct_stream: bool,
    pub supports_direct_play: bool,
    pub supports_transcoding: bool,
    pub protocol: String,
    #[serde(rename = "Type")]
    pub media_source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_stream_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_streams: Option<Vec<MediaStreamDto>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct MediaStreamDto {
    #[serde(rename = "Type")]
    pub stream_type: String,
    pub index: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_title: Option<String>,
    pub is_default: bool,
    pub is_forced: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ItemsResult {
    pub items: Vec<BaseItemDto>,
    pub total_record_count: usize,
    pub start_index: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SearchHintResult {
    pub search_hints: Vec<SearchHint>,
    pub total_record_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SearchHint {
    pub id: String,
    pub name: String,
    #[serde(rename = "Type")]
    pub item_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub production_year: Option<i32>,
}

/// Convert a DB Item into a BaseItemDto.
pub fn item_to_dto(
    item: &sf_db::models::Item,
    images: &[sf_db::models::Image],
    user_data: Option<&UserItemData>,
) -> BaseItemDto {
    let item_type = match item.item_kind.as_str() {
        "series" => "Series",
        "season" => "Season",
        "episode" => "Episode",
        _ => "Movie",
    };

    let is_playable = item.item_kind == "movie" || item.item_kind == "episode";
    let is_folder = item.item_kind == "series" || item.item_kind == "season";

    let run_time_ticks = item
        .runtime_minutes
        .map(|m| (m as i64) * 60 * TICKS_PER_SECOND);

    let mut image_tags = HashMap::new();
    let mut backdrop_image_tags = Vec::new();
    for img in images {
        let tag = img.id.to_string().get(..8).unwrap_or("00000000").to_string();
        match img.image_type.as_str() {
            "primary" => { image_tags.insert("Primary".to_string(), tag); }
            "backdrop" => { backdrop_image_tags.push(tag); }
            _ => { image_tags.insert(img.image_type.clone(), tag); }
        }
    }

    let item_id_str = item.id.to_string();
    let user_data_dto = match user_data {
        Some(ud) => UserDataDto {
            played: ud.completed,
            playback_position_ticks: if ud.position_secs > 0.0 {
                (ud.position_secs * TICKS_PER_SECOND as f64) as i64
            } else {
                0
            },
            play_count: if ud.completed { 1 } else { 0 },
            is_favorite: ud.is_favorite,
            key: item_id_str.clone(),
        },
        None => UserDataDto {
            played: false,
            playback_position_ticks: 0,
            play_count: 0,
            is_favorite: false,
            key: item_id_str.clone(),
        },
    };

    // Use first 8 chars of item ID as etag.
    let etag = item_id_str.get(..8).map(|s| s.to_string());

    BaseItemDto {
        id: item_id_str,
        name: item.name.clone(),
        server_id: "sceneforged-server".to_string(),
        item_type: item_type.to_string(),
        is_folder,
        overview: item.overview.clone(),
        production_year: item.year,
        run_time_ticks,
        community_rating: item.community_rating,
        parent_id: item.parent_id.map(|p| p.to_string()),
        series_id: None,
        series_name: None,
        season_id: None,
        index_number: item.episode_number,
        parent_index_number: item.season_number,
        image_tags,
        backdrop_image_tags,
        user_data: user_data_dto,
        media_sources: None,
        media_streams: None,
        collection_type: None,
        media_type: if is_playable { Some("Video".to_string()) } else { None },
        location_type: "FileSystem".to_string(),
        video_type: if is_playable { Some("VideoFile".to_string()) } else { None },
        child_count: None,
        recursive_item_count: None,
        date_created: Some(item.created_at.clone()),
        etag,
        sort_name: item.sort_name.clone(),
        path: None,
        provider_ids: HashMap::new(),
        genres: Vec::new(),
    }
}
