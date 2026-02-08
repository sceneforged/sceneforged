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
    #[serde(rename = "Type")]
    pub item_type: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_tags: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_data: Option<UserDataDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_sources: Option<Vec<MediaSourceDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_streams: Option<Vec<MediaStreamDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection_type: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserDataDto {
    pub played: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playback_position_ticks: Option<i64>,
    pub is_favorite: bool,
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

    let run_time_ticks = item
        .runtime_minutes
        .map(|m| (m as i64) * 60 * TICKS_PER_SECOND);

    let mut image_tags = HashMap::new();
    for img in images {
        let tag = img.id.to_string().get(..8).unwrap_or("00000000").to_string();
        match img.image_type.as_str() {
            "primary" => { image_tags.insert("Primary".to_string(), tag); }
            "backdrop" => { image_tags.insert("Backdrop".to_string(), tag); }
            _ => { image_tags.insert(img.image_type.clone(), tag); }
        }
    }

    let user_data_dto = user_data.map(|ud| UserDataDto {
        played: ud.completed,
        playback_position_ticks: if ud.position_secs > 0.0 {
            Some((ud.position_secs * TICKS_PER_SECOND as f64) as i64)
        } else {
            None
        },
        is_favorite: ud.is_favorite,
    });

    BaseItemDto {
        id: item.id.to_string(),
        name: item.name.clone(),
        item_type: item_type.to_string(),
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
        image_tags: if image_tags.is_empty() { None } else { Some(image_tags) },
        user_data: user_data_dto,
        media_sources: None,
        media_streams: None,
        collection_type: None,
    }
}
