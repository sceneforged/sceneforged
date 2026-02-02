//! Item database queries.
//!
//! This module provides CRUD operations for media items (movies, series, episodes, etc.),
//! as well as hierarchical queries, search, and media stream/image management.

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use sceneforged_common::{
    Error, ImageId, ItemId, ItemKind, LibraryId, MediaFileId, Result, UserId,
};
use uuid::Uuid;

#[cfg(test)]
use crate::models::ProviderIds;
use crate::models::{Image, Item, MediaStream};

/// Filter options for querying items.
#[derive(Debug, Clone, Default)]
pub struct ItemFilter {
    pub library_id: Option<LibraryId>,
    pub parent_id: Option<ItemId>,
    pub item_kinds: Option<Vec<ItemKind>>,
    pub search_term: Option<String>,
    pub is_favorite: Option<bool>,
    pub user_id: Option<UserId>,
}

/// Pagination options.
#[derive(Debug, Clone, Copy)]
pub struct Pagination {
    pub offset: u32,
    pub limit: u32,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: 100,
        }
    }
}

/// Sort field for items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    Name,
    DateCreated,
    DateModified,
    PremiereDate,
    ProductionYear,
    CommunityRating,
    Random,
}

/// Sort options.
#[derive(Debug, Clone, Copy)]
pub struct SortOptions {
    pub field: SortField,
    pub descending: bool,
}

impl Default for SortOptions {
    fn default() -> Self {
        Self {
            field: SortField::Name,
            descending: false,
        }
    }
}

/// Insert or update an item.
///
/// If an item with the same ID exists, it will be updated. Otherwise, a new item is created.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `item` - Item to upsert
///
/// # Returns
///
/// * `Ok(())` - If the operation succeeded
/// * `Err(Error)` - If a database error occurs
pub fn upsert_item(conn: &Connection, item: &Item) -> Result<()> {
    let genres_json =
        serde_json::to_string(&item.genres).map_err(|e| Error::internal(e.to_string()))?;
    let tags_json =
        serde_json::to_string(&item.tags).map_err(|e| Error::internal(e.to_string()))?;
    let studios_json =
        serde_json::to_string(&item.studios).map_err(|e| Error::internal(e.to_string()))?;
    let people_json =
        serde_json::to_string(&item.people).map_err(|e| Error::internal(e.to_string()))?;
    let provider_ids_json =
        serde_json::to_string(&item.provider_ids).map_err(|e| Error::internal(e.to_string()))?;

    conn.execute(
        "INSERT INTO items (
            id, library_id, parent_id, item_kind, name, sort_name, original_title,
            file_path, container, video_codec, audio_codec, resolution, runtime_ticks,
            size_bytes, overview, tagline, genres, tags, studios, people,
            community_rating, critic_rating, production_year, premiere_date, end_date,
            official_rating, provider_ids, scene_release_name, scene_group,
            index_number, parent_index_number, etag, date_created, date_modified,
            hdr_type, dolby_vision_profile
         ) VALUES (
            :id, :library_id, :parent_id, :item_kind, :name, :sort_name, :original_title,
            :file_path, :container, :video_codec, :audio_codec, :resolution, :runtime_ticks,
            :size_bytes, :overview, :tagline, :genres, :tags, :studios, :people,
            :community_rating, :critic_rating, :production_year, :premiere_date, :end_date,
            :official_rating, :provider_ids, :scene_release_name, :scene_group,
            :index_number, :parent_index_number, :etag, :date_created, :date_modified,
            :hdr_type, :dolby_vision_profile
         )
         ON CONFLICT(id) DO UPDATE SET
            library_id = :library_id,
            parent_id = :parent_id,
            item_kind = :item_kind,
            name = :name,
            sort_name = :sort_name,
            original_title = :original_title,
            file_path = :file_path,
            container = :container,
            video_codec = :video_codec,
            audio_codec = :audio_codec,
            resolution = :resolution,
            runtime_ticks = :runtime_ticks,
            size_bytes = :size_bytes,
            overview = :overview,
            tagline = :tagline,
            genres = :genres,
            tags = :tags,
            studios = :studios,
            people = :people,
            community_rating = :community_rating,
            critic_rating = :critic_rating,
            production_year = :production_year,
            premiere_date = :premiere_date,
            end_date = :end_date,
            official_rating = :official_rating,
            provider_ids = :provider_ids,
            scene_release_name = :scene_release_name,
            scene_group = :scene_group,
            index_number = :index_number,
            parent_index_number = :parent_index_number,
            etag = :etag,
            date_modified = :date_modified,
            hdr_type = :hdr_type,
            dolby_vision_profile = :dolby_vision_profile",
        rusqlite::named_params! {
            ":id": item.id.to_string(),
            ":library_id": item.library_id.to_string(),
            ":parent_id": item.parent_id.map(|id| id.to_string()),
            ":item_kind": item.item_kind.to_string(),
            ":name": &item.name,
            ":sort_name": &item.sort_name,
            ":original_title": &item.original_title,
            ":file_path": &item.file_path,
            ":container": &item.container,
            ":video_codec": &item.video_codec,
            ":audio_codec": &item.audio_codec,
            ":resolution": &item.resolution,
            ":runtime_ticks": item.runtime_ticks,
            ":size_bytes": item.size_bytes,
            ":overview": &item.overview,
            ":tagline": &item.tagline,
            ":genres": genres_json,
            ":tags": tags_json,
            ":studios": studios_json,
            ":people": people_json,
            ":community_rating": item.community_rating,
            ":critic_rating": item.critic_rating,
            ":production_year": item.production_year,
            ":premiere_date": &item.premiere_date,
            ":end_date": &item.end_date,
            ":official_rating": &item.official_rating,
            ":provider_ids": provider_ids_json,
            ":scene_release_name": &item.scene_release_name,
            ":scene_group": &item.scene_group,
            ":index_number": item.index_number,
            ":parent_index_number": item.parent_index_number,
            ":etag": &item.etag,
            ":date_created": item.date_created.to_rfc3339(),
            ":date_modified": item.date_modified.to_rfc3339(),
            ":hdr_type": &item.hdr_type,
            ":dolby_vision_profile": &item.dolby_vision_profile,
        },
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(())
}

/// Parse an item from a database row.
fn parse_item_row(row: &rusqlite::Row) -> rusqlite::Result<Item> {
    let genres_json: String = row.get(16)?;
    let tags_json: String = row.get(17)?;
    let studios_json: String = row.get(18)?;
    let people_json: String = row.get(19)?;
    let provider_ids_json: String = row.get(26)?;

    Ok(Item {
        id: ItemId::from(Uuid::parse_str(&row.get::<_, String>(0)?).unwrap()),
        library_id: LibraryId::from(Uuid::parse_str(&row.get::<_, String>(1)?).unwrap()),
        parent_id: row
            .get::<_, Option<String>>(2)?
            .map(|s| ItemId::from(Uuid::parse_str(&s).unwrap())),
        item_kind: serde_json::from_str(&format!("\"{}\"", row.get::<_, String>(3)?)).unwrap(),
        name: row.get(4)?,
        sort_name: row.get(5)?,
        original_title: row.get(6)?,
        file_path: row.get(7)?,
        container: row.get(8)?,
        video_codec: row.get(9)?,
        audio_codec: row.get(10)?,
        resolution: row.get(11)?,
        runtime_ticks: row.get(12)?,
        size_bytes: row.get(13)?,
        overview: row.get(14)?,
        tagline: row.get(15)?,
        genres: serde_json::from_str(&genres_json).unwrap_or_default(),
        tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        studios: serde_json::from_str(&studios_json).unwrap_or_default(),
        people: serde_json::from_str(&people_json).unwrap_or_default(),
        community_rating: row.get(20)?,
        critic_rating: row.get(21)?,
        production_year: row.get(22)?,
        premiere_date: row.get(23)?,
        end_date: row.get(24)?,
        official_rating: row.get(25)?,
        provider_ids: serde_json::from_str(&provider_ids_json).unwrap_or_default(),
        scene_release_name: row.get(27)?,
        scene_group: row.get(28)?,
        index_number: row.get(29)?,
        parent_index_number: row.get(30)?,
        etag: row.get(31)?,
        date_created: DateTime::parse_from_rfc3339(&row.get::<_, String>(32)?)
            .unwrap()
            .with_timezone(&Utc),
        date_modified: DateTime::parse_from_rfc3339(&row.get::<_, String>(33)?)
            .unwrap()
            .with_timezone(&Utc),
        hdr_type: row.get(34)?,
        dolby_vision_profile: row.get(35)?,
    })
}

/// Get an item by ID.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `id` - Item ID
///
/// # Returns
///
/// * `Ok(Some(Item))` - The item if found
/// * `Ok(None)` - If the item does not exist
/// * `Err(Error)` - If a database error occurs
pub fn get_item(conn: &Connection, id: ItemId) -> Result<Option<Item>> {
    let result = conn.query_row(
        "SELECT id, library_id, parent_id, item_kind, name, sort_name, original_title,
                file_path, container, video_codec, audio_codec, resolution, runtime_ticks,
                size_bytes, overview, tagline, genres, tags, studios, people,
                community_rating, critic_rating, production_year, premiere_date, end_date,
                official_rating, provider_ids, scene_release_name, scene_group,
                index_number, parent_index_number, etag, date_created, date_modified,
                hdr_type, dolby_vision_profile
         FROM items WHERE id = :id",
        rusqlite::named_params! { ":id": id.to_string() },
        parse_item_row,
    );

    match result {
        Ok(item) => Ok(Some(item)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Get an item by file path.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `path` - File path to search for
///
/// # Returns
///
/// * `Ok(Some(Item))` - The item if found
/// * `Ok(None)` - If no item with this path exists
/// * `Err(Error)` - If a database error occurs
pub fn get_item_by_path(conn: &Connection, path: &str) -> Result<Option<Item>> {
    let result = conn.query_row(
        "SELECT id, library_id, parent_id, item_kind, name, sort_name, original_title,
                file_path, container, video_codec, audio_codec, resolution, runtime_ticks,
                size_bytes, overview, tagline, genres, tags, studios, people,
                community_rating, critic_rating, production_year, premiere_date, end_date,
                official_rating, provider_ids, scene_release_name, scene_group,
                index_number, parent_index_number, etag, date_created, date_modified,
                hdr_type, dolby_vision_profile
         FROM items WHERE file_path = :path",
        rusqlite::named_params! { ":path": path },
        parse_item_row,
    );

    match result {
        Ok(item) => Ok(Some(item)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// List items with filtering, sorting, and pagination.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `filter` - Filter options
/// * `sort` - Sort options
/// * `pagination` - Pagination options
///
/// # Returns
///
/// * `Ok(Vec<Item>)` - List of items matching the criteria
/// * `Err(Error)` - If a database error occurs
pub fn list_items(
    conn: &Connection,
    filter: &ItemFilter,
    sort: &SortOptions,
    pagination: &Pagination,
) -> Result<Vec<Item>> {
    let mut query = String::from(
        "SELECT DISTINCT i.id, i.library_id, i.parent_id, i.item_kind, i.name, i.sort_name, i.original_title,
                i.file_path, i.container, i.video_codec, i.audio_codec, i.resolution, i.runtime_ticks,
                i.size_bytes, i.overview, i.tagline, i.genres, i.tags, i.studios, i.people,
                i.community_rating, i.critic_rating, i.production_year, i.premiere_date, i.end_date,
                i.official_rating, i.provider_ids, i.scene_release_name, i.scene_group,
                i.index_number, i.parent_index_number, i.etag, i.date_created, i.date_modified,
                i.hdr_type, i.dolby_vision_profile
         FROM items i",
    );

    // Join with user_item_data if filtering by favorites
    if filter.is_favorite.is_some() && filter.user_id.is_some() {
        query.push_str(" LEFT JOIN user_item_data uid ON i.id = uid.item_id");
    }

    query.push_str(" WHERE 1=1");

    // Build WHERE clause
    if filter.library_id.is_some() {
        query.push_str(" AND i.library_id = :library_id");
    }

    if filter.parent_id.is_some() {
        query.push_str(" AND i.parent_id = :parent_id");
    }

    if let Some(ref kinds) = filter.item_kinds {
        if !kinds.is_empty() {
            let placeholders: Vec<_> = kinds.iter().map(|_| "?").collect();
            query.push_str(&format!(
                " AND i.item_kind IN ({})",
                placeholders.join(", ")
            ));
        }
    }

    if filter.search_term.is_some() {
        query.push_str(" AND (i.name LIKE :search OR i.original_title LIKE :search)");
    }

    if filter.is_favorite == Some(true) && filter.user_id.is_some() {
        query.push_str(" AND uid.user_id = :user_id AND uid.is_favorite = 1");
    }

    // Add ORDER BY clause
    query.push_str(" ORDER BY ");
    match sort.field {
        SortField::Name => query.push_str("COALESCE(i.sort_name, i.name)"),
        SortField::DateCreated => query.push_str("i.date_created"),
        SortField::DateModified => query.push_str("i.date_modified"),
        SortField::PremiereDate => query.push_str("i.premiere_date"),
        SortField::ProductionYear => query.push_str("i.production_year"),
        SortField::CommunityRating => query.push_str("i.community_rating"),
        SortField::Random => query.push_str("RANDOM()"),
    }

    if sort.descending {
        query.push_str(" DESC");
    } else {
        query.push_str(" ASC");
    }

    // Add LIMIT and OFFSET
    query.push_str(" LIMIT :limit OFFSET :offset");

    let mut stmt = conn
        .prepare(&query)
        .map_err(|e| Error::database(e.to_string()))?;

    // Bind parameters
    let mut params: Vec<(&str, &dyn rusqlite::ToSql)> = vec![
        (":limit", &pagination.limit),
        (":offset", &pagination.offset),
    ];

    let library_id_str = filter.library_id.map(|id| id.to_string());
    if let Some(ref id) = library_id_str {
        params.push((":library_id", id));
    }

    let parent_id_str = filter.parent_id.map(|id| id.to_string());
    if let Some(ref id) = parent_id_str {
        params.push((":parent_id", id));
    }

    let search_pattern = filter.search_term.as_ref().map(|s| format!("%{}%", s));
    if let Some(ref pattern) = search_pattern {
        params.push((":search", pattern));
    }

    let user_id_str = filter.user_id.map(|id| id.to_string());
    if let Some(ref id) = user_id_str {
        params.push((":user_id", id));
    }

    let items = stmt
        .query_map(&*params, parse_item_row)
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(items)
}

/// Count items matching filter criteria.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `filter` - Filter options
///
/// # Returns
///
/// * `Ok(u32)` - Number of items matching the filter
/// * `Err(Error)` - If a database error occurs
pub fn count_items(conn: &Connection, filter: &ItemFilter) -> Result<u32> {
    let mut query = String::from("SELECT COUNT(DISTINCT i.id) FROM items i");

    if filter.is_favorite.is_some() && filter.user_id.is_some() {
        query.push_str(" LEFT JOIN user_item_data uid ON i.id = uid.item_id");
    }

    query.push_str(" WHERE 1=1");

    if filter.library_id.is_some() {
        query.push_str(" AND i.library_id = :library_id");
    }

    if filter.parent_id.is_some() {
        query.push_str(" AND i.parent_id = :parent_id");
    }

    if filter.search_term.is_some() {
        query.push_str(" AND (i.name LIKE :search OR i.original_title LIKE :search)");
    }

    if filter.is_favorite == Some(true) && filter.user_id.is_some() {
        query.push_str(" AND uid.user_id = :user_id AND uid.is_favorite = 1");
    }

    let mut stmt = conn
        .prepare(&query)
        .map_err(|e| Error::database(e.to_string()))?;

    let library_id_str = filter.library_id.map(|id| id.to_string());
    let parent_id_str = filter.parent_id.map(|id| id.to_string());
    let search_pattern = filter.search_term.as_ref().map(|s| format!("%{}%", s));
    let user_id_str = filter.user_id.map(|id| id.to_string());

    let mut params: Vec<(&str, &dyn rusqlite::ToSql)> = vec![];
    if let Some(ref id) = library_id_str {
        params.push((":library_id", id));
    }
    if let Some(ref id) = parent_id_str {
        params.push((":parent_id", id));
    }
    if let Some(ref pattern) = search_pattern {
        params.push((":search", pattern));
    }
    if let Some(ref id) = user_id_str {
        params.push((":user_id", id));
    }

    let count: i64 = stmt
        .query_row(&*params, |row| row.get(0))
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(count as u32)
}

/// Get children of an item (seasons of series, episodes of season).
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `parent_id` - Parent item ID
///
/// # Returns
///
/// * `Ok(Vec<Item>)` - List of child items, sorted by index_number
/// * `Err(Error)` - If a database error occurs
pub fn get_children(conn: &Connection, parent_id: ItemId) -> Result<Vec<Item>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, library_id, parent_id, item_kind, name, sort_name, original_title,
                    file_path, container, video_codec, audio_codec, resolution, runtime_ticks,
                    size_bytes, overview, tagline, genres, tags, studios, people,
                    community_rating, critic_rating, production_year, premiere_date, end_date,
                    official_rating, provider_ids, scene_release_name, scene_group,
                    index_number, parent_index_number, etag, date_created, date_modified,
                    hdr_type, dolby_vision_profile
             FROM items
             WHERE parent_id = :parent_id
             ORDER BY index_number ASC, name ASC",
        )
        .map_err(|e| Error::database(e.to_string()))?;

    let items = stmt
        .query_map(
            rusqlite::named_params! { ":parent_id": parent_id.to_string() },
            parse_item_row,
        )
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(items)
}

/// Get recently added items.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `library_id` - Optional library ID to filter by
/// * `limit` - Maximum number of items to return
///
/// # Returns
///
/// * `Ok(Vec<Item>)` - List of recently added items
/// * `Err(Error)` - If a database error occurs
pub fn get_recent_items(
    conn: &Connection,
    library_id: Option<LibraryId>,
    limit: u32,
) -> Result<Vec<Item>> {
    let query = if library_id.is_some() {
        "SELECT id, library_id, parent_id, item_kind, name, sort_name, original_title,
                file_path, container, video_codec, audio_codec, resolution, runtime_ticks,
                size_bytes, overview, tagline, genres, tags, studios, people,
                community_rating, critic_rating, production_year, premiere_date, end_date,
                official_rating, provider_ids, scene_release_name, scene_group,
                index_number, parent_index_number, etag, date_created, date_modified,
                hdr_type, dolby_vision_profile
         FROM items
         WHERE library_id = :library_id
         ORDER BY date_created DESC
         LIMIT :limit"
    } else {
        "SELECT id, library_id, parent_id, item_kind, name, sort_name, original_title,
                file_path, container, video_codec, audio_codec, resolution, runtime_ticks,
                size_bytes, overview, tagline, genres, tags, studios, people,
                community_rating, critic_rating, production_year, premiere_date, end_date,
                official_rating, provider_ids, scene_release_name, scene_group,
                index_number, parent_index_number, etag, date_created, date_modified,
                hdr_type, dolby_vision_profile
         FROM items
         ORDER BY date_created DESC
         LIMIT :limit"
    };

    let mut stmt = conn
        .prepare(query)
        .map_err(|e| Error::database(e.to_string()))?;

    let items = if let Some(lib_id) = library_id {
        stmt.query_map(
            rusqlite::named_params! {
                ":library_id": lib_id.to_string(),
                ":limit": limit,
            },
            parse_item_row,
        )
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?
    } else {
        stmt.query_map(rusqlite::named_params! { ":limit": limit }, parse_item_row)
            .map_err(|e| Error::database(e.to_string()))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::database(e.to_string()))?
    };

    Ok(items)
}

/// Delete an item.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `id` - Item ID to delete
///
/// # Returns
///
/// * `Ok(true)` - If the item was deleted
/// * `Ok(false)` - If the item did not exist
/// * `Err(Error)` - If a database error occurs
pub fn delete_item(conn: &Connection, id: ItemId) -> Result<bool> {
    let rows_affected = conn
        .execute(
            "DELETE FROM items WHERE id = :id",
            rusqlite::named_params! { ":id": id.to_string() },
        )
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(rows_affected > 0)
}

/// Insert or update media streams for an item.
///
/// This will delete all existing streams for the media file and insert the new ones.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `media_file_id` - Media file ID
/// * `streams` - List of media streams
///
/// # Returns
///
/// * `Ok(())` - If the operation succeeded
/// * `Err(Error)` - If a database error occurs
pub fn upsert_media_streams(
    conn: &Connection,
    media_file_id: MediaFileId,
    streams: &[MediaStream],
) -> Result<()> {
    // Delete existing streams
    conn.execute(
        "DELETE FROM media_streams WHERE media_file_id = :media_file_id",
        rusqlite::named_params! { ":media_file_id": media_file_id.to_string() },
    )
    .map_err(|e| Error::database(e.to_string()))?;

    // Insert new streams
    for stream in streams {
        conn.execute(
            "INSERT INTO media_streams (
                id, media_file_id, stream_type, index_num, codec, language, title,
                is_default, is_forced, is_external, external_path,
                width, height, bit_rate, frame_rate, pixel_format,
                color_primaries, color_transfer, color_space,
                channels, channel_layout, sample_rate
             ) VALUES (
                :id, :media_file_id, :stream_type, :index_num, :codec, :language, :title,
                :is_default, :is_forced, :is_external, :external_path,
                :width, :height, :bit_rate, :frame_rate, :pixel_format,
                :color_primaries, :color_transfer, :color_space,
                :channels, :channel_layout, :sample_rate
             )",
            rusqlite::named_params! {
                ":id": &stream.id,
                ":media_file_id": media_file_id.to_string(),
                ":stream_type": stream.stream_type.to_string(),
                ":index_num": stream.index_num,
                ":codec": &stream.codec,
                ":language": &stream.language,
                ":title": &stream.title,
                ":is_default": stream.is_default,
                ":is_forced": stream.is_forced,
                ":is_external": stream.is_external,
                ":external_path": &stream.external_path,
                ":width": stream.width,
                ":height": stream.height,
                ":bit_rate": stream.bit_rate,
                ":frame_rate": stream.frame_rate,
                ":pixel_format": &stream.pixel_format,
                ":color_primaries": &stream.color_primaries,
                ":color_transfer": &stream.color_transfer,
                ":color_space": &stream.color_space,
                ":channels": stream.channels,
                ":channel_layout": &stream.channel_layout,
                ":sample_rate": stream.sample_rate,
            },
        )
        .map_err(|e| Error::database(e.to_string()))?;
    }

    Ok(())
}

/// Get media streams for a media file.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `media_file_id` - Media file ID
///
/// # Returns
///
/// * `Ok(Vec<MediaStream>)` - List of media streams, sorted by index_num
/// * `Err(Error)` - If a database error occurs
pub fn get_media_streams(
    conn: &Connection,
    media_file_id: MediaFileId,
) -> Result<Vec<MediaStream>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, media_file_id, stream_type, index_num, codec, language, title,
                    is_default, is_forced, is_external, external_path,
                    width, height, bit_rate, frame_rate, pixel_format,
                    color_primaries, color_transfer, color_space,
                    channels, channel_layout, sample_rate
             FROM media_streams
             WHERE media_file_id = :media_file_id
             ORDER BY index_num ASC",
        )
        .map_err(|e| Error::database(e.to_string()))?;

    let streams = stmt
        .query_map(
            rusqlite::named_params! { ":media_file_id": media_file_id.to_string() },
            |row| {
                Ok(MediaStream {
                    id: row.get(0)?,
                    media_file_id: MediaFileId::from(
                        Uuid::parse_str(&row.get::<_, String>(1)?).unwrap(),
                    ),
                    stream_type: serde_json::from_str(&format!("\"{}\"", row.get::<_, String>(2)?))
                        .unwrap(),
                    index_num: row.get(3)?,
                    codec: row.get(4)?,
                    language: row.get(5)?,
                    title: row.get(6)?,
                    is_default: row.get::<_, i32>(7)? != 0,
                    is_forced: row.get::<_, i32>(8)? != 0,
                    is_external: row.get::<_, i32>(9)? != 0,
                    external_path: row.get(10)?,
                    width: row.get(11)?,
                    height: row.get(12)?,
                    bit_rate: row.get(13)?,
                    frame_rate: row.get(14)?,
                    pixel_format: row.get(15)?,
                    color_primaries: row.get(16)?,
                    color_transfer: row.get(17)?,
                    color_space: row.get(18)?,
                    channels: row.get(19)?,
                    channel_layout: row.get(20)?,
                    sample_rate: row.get(21)?,
                })
            },
        )
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(streams)
}

/// Insert or update images for an item.
///
/// This will delete all existing images for the item and insert the new ones.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `item_id` - Item ID
/// * `images` - List of images
///
/// # Returns
///
/// * `Ok(())` - If the operation succeeded
/// * `Err(Error)` - If a database error occurs
pub fn upsert_images(conn: &Connection, item_id: ItemId, images: &[Image]) -> Result<()> {
    // Delete existing images
    conn.execute(
        "DELETE FROM images WHERE item_id = :item_id",
        rusqlite::named_params! { ":item_id": item_id.to_string() },
    )
    .map_err(|e| Error::database(e.to_string()))?;

    // Insert new images
    for image in images {
        conn.execute(
            "INSERT INTO images (id, item_id, image_type, path, provider, width, height, tag)
             VALUES (:id, :item_id, :image_type, :path, :provider, :width, :height, :tag)",
            rusqlite::named_params! {
                ":id": image.id.to_string(),
                ":item_id": item_id.to_string(),
                ":image_type": image.image_type.to_string(),
                ":path": &image.path,
                ":provider": &image.provider,
                ":width": image.width,
                ":height": image.height,
                ":tag": &image.tag,
            },
        )
        .map_err(|e| Error::database(e.to_string()))?;
    }

    Ok(())
}

/// Get images for an item.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `item_id` - Item ID
///
/// # Returns
///
/// * `Ok(Vec<Image>)` - List of images
/// * `Err(Error)` - If a database error occurs
pub fn get_images(conn: &Connection, item_id: ItemId) -> Result<Vec<Image>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, item_id, image_type, path, provider, width, height, tag
             FROM images
             WHERE item_id = :item_id",
        )
        .map_err(|e| Error::database(e.to_string()))?;

    let images = stmt
        .query_map(
            rusqlite::named_params! { ":item_id": item_id.to_string() },
            |row| {
                Ok(Image {
                    id: ImageId::from(Uuid::parse_str(&row.get::<_, String>(0)?).unwrap()),
                    item_id: ItemId::from(Uuid::parse_str(&row.get::<_, String>(1)?).unwrap()),
                    image_type: serde_json::from_str(&format!("\"{}\"", row.get::<_, String>(2)?))
                        .unwrap(),
                    path: row.get(3)?,
                    provider: row.get(4)?,
                    width: row.get(5)?,
                    height: row.get(6)?,
                    tag: row.get(7)?,
                })
            },
        )
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(images)
}

/// Search items by name.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `query` - Search query
/// * `limit` - Maximum number of results
///
/// # Returns
///
/// * `Ok(Vec<Item>)` - List of matching items
/// * `Err(Error)` - If a database error occurs
pub fn search_items(conn: &Connection, query: &str, limit: u32) -> Result<Vec<Item>> {
    let pattern = format!("%{}%", query);

    let mut stmt = conn
        .prepare(
            "SELECT id, library_id, parent_id, item_kind, name, sort_name, original_title,
                    file_path, container, video_codec, audio_codec, resolution, runtime_ticks,
                    size_bytes, overview, tagline, genres, tags, studios, people,
                    community_rating, critic_rating, production_year, premiere_date, end_date,
                    official_rating, provider_ids, scene_release_name, scene_group,
                    index_number, parent_index_number, etag, date_created, date_modified,
                    hdr_type, dolby_vision_profile
             FROM items
             WHERE name LIKE :pattern OR original_title LIKE :pattern
             ORDER BY name ASC
             LIMIT :limit",
        )
        .map_err(|e| Error::database(e.to_string()))?;

    let items = stmt
        .query_map(
            rusqlite::named_params! {
                ":pattern": &pattern,
                ":limit": limit,
            },
            parse_item_row,
        )
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_memory_pool;
    use crate::queries::libraries::create_library;
    use sceneforged_common::{ImageType, MediaType, StreamType};

    fn create_test_library(conn: &Connection) -> LibraryId {
        create_library(conn, "Test Library", MediaType::Movies, &[])
            .unwrap()
            .id
    }

    fn create_test_item(conn: &Connection, library_id: LibraryId, name: &str) -> Item {
        let item = Item {
            id: ItemId::new(),
            library_id,
            parent_id: None,
            item_kind: ItemKind::Movie,
            name: name.to_string(),
            sort_name: None,
            original_title: None,
            file_path: Some(format!("/media/{}.mkv", name)),
            container: Some("mkv".to_string()),
            video_codec: Some("hevc".to_string()),
            audio_codec: Some("aac".to_string()),
            resolution: Some("1920x1080".to_string()),
            runtime_ticks: Some(72000000000),
            size_bytes: Some(1024 * 1024 * 1024),
            overview: Some("Test overview".to_string()),
            tagline: None,
            genres: vec!["Action".to_string()],
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

        upsert_item(conn, &item).unwrap();
        item
    }

    #[test]
    fn test_upsert_item() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);

        let item = create_test_item(&conn, library_id, "Test Movie");
        let found = get_item(&conn, item.id).unwrap().unwrap();

        assert_eq!(found.name, "Test Movie");
        assert_eq!(found.item_kind, ItemKind::Movie);
    }

    #[test]
    fn test_get_item_by_path() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);

        create_test_item(&conn, library_id, "Test Movie");
        let found = get_item_by_path(&conn, "/media/Test Movie.mkv").unwrap();

        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Test Movie");
    }

    #[test]
    fn test_list_items() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);

        create_test_item(&conn, library_id, "Movie A");
        create_test_item(&conn, library_id, "Movie B");
        create_test_item(&conn, library_id, "Movie C");

        let filter = ItemFilter {
            library_id: Some(library_id),
            ..Default::default()
        };
        let sort = SortOptions::default();
        let pagination = Pagination::default();

        let items = list_items(&conn, &filter, &sort, &pagination).unwrap();
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn test_count_items() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);

        create_test_item(&conn, library_id, "Movie A");
        create_test_item(&conn, library_id, "Movie B");

        let filter = ItemFilter {
            library_id: Some(library_id),
            ..Default::default()
        };

        let count = count_items(&conn, &filter).unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_get_children() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);

        let mut parent = create_test_item(&conn, library_id, "Series");
        parent.item_kind = ItemKind::Series;
        upsert_item(&conn, &parent).unwrap();

        let mut child1 = create_test_item(&conn, library_id, "Season 1");
        child1.item_kind = ItemKind::Season;
        child1.parent_id = Some(parent.id);
        child1.index_number = Some(1);
        upsert_item(&conn, &child1).unwrap();

        let mut child2 = create_test_item(&conn, library_id, "Season 2");
        child2.item_kind = ItemKind::Season;
        child2.parent_id = Some(parent.id);
        child2.index_number = Some(2);
        upsert_item(&conn, &child2).unwrap();

        let children = get_children(&conn, parent.id).unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].index_number, Some(1));
        assert_eq!(children[1].index_number, Some(2));
    }

    #[test]
    fn test_delete_item() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);

        let item = create_test_item(&conn, library_id, "Test Movie");
        let deleted = delete_item(&conn, item.id).unwrap();
        assert!(deleted);

        let found = get_item(&conn, item.id).unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_upsert_media_streams() {
        use crate::queries::media_files::create_media_file;
        use sceneforged_common::FileRole;

        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);
        let item = create_test_item(&conn, library_id, "Test Movie");

        // Create a media file first (streams belong to files, not items)
        let media_file = create_media_file(
            &conn,
            item.id,
            FileRole::Source,
            "/media/movie.mkv",
            1024,
            "mkv",
        )
        .unwrap();

        let streams = vec![MediaStream {
            id: uuid::Uuid::new_v4().to_string(),
            media_file_id: media_file.id,
            stream_type: StreamType::Video,
            index_num: 0,
            codec: Some("hevc".to_string()),
            language: None,
            title: None,
            is_default: true,
            is_forced: false,
            is_external: false,
            external_path: None,
            width: Some(1920),
            height: Some(1080),
            bit_rate: None,
            frame_rate: Some(23.976),
            pixel_format: None,
            color_primaries: None,
            color_transfer: None,
            color_space: None,
            channels: None,
            channel_layout: None,
            sample_rate: None,
        }];

        upsert_media_streams(&conn, media_file.id, &streams).unwrap();
        let found_streams = get_media_streams(&conn, media_file.id).unwrap();

        assert_eq!(found_streams.len(), 1);
        assert_eq!(found_streams[0].stream_type, StreamType::Video);
    }

    #[test]
    fn test_upsert_images() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);
        let item = create_test_item(&conn, library_id, "Test Movie");

        let images = vec![Image {
            id: ImageId::new(),
            item_id: item.id,
            image_type: ImageType::Primary,
            path: "/images/poster.jpg".to_string(),
            provider: Some("tmdb".to_string()),
            width: Some(1000),
            height: Some(1500),
            tag: None,
        }];

        upsert_images(&conn, item.id, &images).unwrap();
        let found_images = get_images(&conn, item.id).unwrap();

        assert_eq!(found_images.len(), 1);
        assert_eq!(found_images[0].image_type, ImageType::Primary);
    }

    #[test]
    fn test_search_items() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);

        create_test_item(&conn, library_id, "The Matrix");
        create_test_item(&conn, library_id, "Matrix Reloaded");
        create_test_item(&conn, library_id, "Inception");

        let results = search_items(&conn, "Matrix", 10).unwrap();
        assert_eq!(results.len(), 2);
    }
}
