//! Playback database queries.
//!
//! This module provides operations for managing user-specific item data,
//! including playback position, play count, favorites, and resume functionality.

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use sceneforged_common::{Error, ItemId, Result, UserId};
use uuid::Uuid;

use crate::models::{Item, UserItemData};
use sceneforged_common::LibraryId;

/// Get or create user item data.
///
/// If user item data doesn't exist for this user/item combination, it will be created
/// with default values (position 0, not played, not favorite).
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `user_id` - User ID
/// * `item_id` - Item ID
///
/// # Returns
///
/// * `Ok(UserItemData)` - The user item data
/// * `Err(Error)` - If a database error occurs
pub fn get_user_item_data(
    conn: &Connection,
    user_id: UserId,
    item_id: ItemId,
) -> Result<UserItemData> {
    let result = conn.query_row(
        "SELECT user_id, item_id, playback_position_ticks, play_count, played, is_favorite, last_played_date
         FROM user_item_data
         WHERE user_id = :user_id AND item_id = :item_id",
        rusqlite::named_params! {
            ":user_id": user_id.to_string(),
            ":item_id": item_id.to_string(),
        },
        |row| {
            Ok(UserItemData {
                user_id: UserId::from(Uuid::parse_str(&row.get::<_, String>(0)?).unwrap()),
                item_id: ItemId::from(Uuid::parse_str(&row.get::<_, String>(1)?).unwrap()),
                playback_position_ticks: row.get(2)?,
                play_count: row.get(3)?,
                played: row.get::<_, i32>(4)? != 0,
                is_favorite: row.get::<_, i32>(5)? != 0,
                last_played_date: row.get::<_, Option<String>>(6)?
                    .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Utc)),
            })
        },
    );

    match result {
        Ok(data) => Ok(data),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            // Create default user item data
            let data = UserItemData {
                user_id,
                item_id,
                playback_position_ticks: 0,
                play_count: 0,
                played: false,
                is_favorite: false,
                last_played_date: None,
            };

            conn.execute(
                "INSERT INTO user_item_data (user_id, item_id, playback_position_ticks, play_count, played, is_favorite)
                 VALUES (:user_id, :item_id, 0, 0, 0, 0)",
                rusqlite::named_params! {
                    ":user_id": user_id.to_string(),
                    ":item_id": item_id.to_string(),
                },
            )
            .map_err(|e| Error::database(e.to_string()))?;

            Ok(data)
        }
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Update playback position for a user and item.
///
/// This will also update the last_played_date.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `user_id` - User ID
/// * `item_id` - Item ID
/// * `position_ticks` - New playback position in ticks
///
/// # Returns
///
/// * `Ok(())` - If the update succeeded
/// * `Err(Error)` - If a database error occurs
pub fn update_playback_position(
    conn: &Connection,
    user_id: UserId,
    item_id: ItemId,
    position_ticks: i64,
) -> Result<()> {
    // Ensure user item data exists
    get_user_item_data(conn, user_id, item_id)?;

    let now = Utc::now();

    conn.execute(
        "UPDATE user_item_data
         SET playback_position_ticks = :position_ticks,
             last_played_date = :last_played_date
         WHERE user_id = :user_id AND item_id = :item_id",
        rusqlite::named_params! {
            ":user_id": user_id.to_string(),
            ":item_id": item_id.to_string(),
            ":position_ticks": position_ticks,
            ":last_played_date": now.to_rfc3339(),
        },
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(())
}

/// Mark an item as played for a user.
///
/// This increments the play count, sets played to true, resets position to 0,
/// and updates the last_played_date.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `user_id` - User ID
/// * `item_id` - Item ID
///
/// # Returns
///
/// * `Ok(())` - If the update succeeded
/// * `Err(Error)` - If a database error occurs
pub fn mark_played(conn: &Connection, user_id: UserId, item_id: ItemId) -> Result<()> {
    // Ensure user item data exists
    get_user_item_data(conn, user_id, item_id)?;

    let now = Utc::now();

    conn.execute(
        "UPDATE user_item_data
         SET played = 1,
             play_count = play_count + 1,
             playback_position_ticks = 0,
             last_played_date = :last_played_date
         WHERE user_id = :user_id AND item_id = :item_id",
        rusqlite::named_params! {
            ":user_id": user_id.to_string(),
            ":item_id": item_id.to_string(),
            ":last_played_date": now.to_rfc3339(),
        },
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(())
}

/// Mark an item as unplayed for a user.
///
/// This resets play count to 0, sets played to false, and resets position to 0.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `user_id` - User ID
/// * `item_id` - Item ID
///
/// # Returns
///
/// * `Ok(())` - If the update succeeded
/// * `Err(Error)` - If a database error occurs
pub fn mark_unplayed(conn: &Connection, user_id: UserId, item_id: ItemId) -> Result<()> {
    // Ensure user item data exists
    get_user_item_data(conn, user_id, item_id)?;

    conn.execute(
        "UPDATE user_item_data
         SET played = 0,
             play_count = 0,
             playback_position_ticks = 0
         WHERE user_id = :user_id AND item_id = :item_id",
        rusqlite::named_params! {
            ":user_id": user_id.to_string(),
            ":item_id": item_id.to_string(),
        },
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(())
}

/// Toggle favorite status for a user and item.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `user_id` - User ID
/// * `item_id` - Item ID
///
/// # Returns
///
/// * `Ok(bool)` - The new favorite status (true if now favorite, false if not)
/// * `Err(Error)` - If a database error occurs
pub fn toggle_favorite(conn: &Connection, user_id: UserId, item_id: ItemId) -> Result<bool> {
    let current_data = get_user_item_data(conn, user_id, item_id)?;
    let new_favorite = !current_data.is_favorite;

    conn.execute(
        "UPDATE user_item_data
         SET is_favorite = :is_favorite
         WHERE user_id = :user_id AND item_id = :item_id",
        rusqlite::named_params! {
            ":user_id": user_id.to_string(),
            ":item_id": item_id.to_string(),
            ":is_favorite": new_favorite,
        },
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(new_favorite)
}

/// Set favorite status for a user and item.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `user_id` - User ID
/// * `item_id` - Item ID
/// * `is_favorite` - Whether the item should be marked as favorite
///
/// # Returns
///
/// * `Ok(())` - If the update succeeded
/// * `Err(Error)` - If a database error occurs
pub fn set_favorite(
    conn: &Connection,
    user_id: UserId,
    item_id: ItemId,
    is_favorite: bool,
) -> Result<()> {
    // Ensure user item data exists
    get_user_item_data(conn, user_id, item_id)?;

    conn.execute(
        "UPDATE user_item_data
         SET is_favorite = :is_favorite
         WHERE user_id = :user_id AND item_id = :item_id",
        rusqlite::named_params! {
            ":user_id": user_id.to_string(),
            ":item_id": item_id.to_string(),
            ":is_favorite": is_favorite,
        },
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(())
}

/// Get items in progress for a user (for resume functionality).
///
/// Returns items that have been partially watched (position > 0 and not fully played).
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `user_id` - User ID
/// * `limit` - Maximum number of items to return
///
/// # Returns
///
/// * `Ok(Vec<(Item, UserItemData)>)` - List of items with their user data, sorted by last_played_date
/// * `Err(Error)` - If a database error occurs
pub fn get_in_progress_items(
    conn: &Connection,
    user_id: UserId,
    limit: u32,
) -> Result<Vec<(Item, UserItemData)>> {
    let mut stmt = conn
        .prepare(
            "SELECT i.id, i.library_id, i.parent_id, i.item_kind, i.name, i.sort_name, i.original_title,
                    i.file_path, i.container, i.video_codec, i.audio_codec, i.resolution, i.runtime_ticks,
                    i.size_bytes, i.overview, i.tagline, i.genres, i.tags, i.studios, i.people,
                    i.community_rating, i.critic_rating, i.production_year, i.premiere_date, i.end_date,
                    i.official_rating, i.provider_ids, i.scene_release_name, i.scene_group,
                    i.index_number, i.parent_index_number, i.etag, i.date_created, i.date_modified,
                    i.hdr_type, i.dolby_vision_profile,
                    uid.user_id, uid.item_id, uid.playback_position_ticks, uid.play_count,
                    uid.played, uid.is_favorite, uid.last_played_date
             FROM items i
             INNER JOIN user_item_data uid ON i.id = uid.item_id
             WHERE uid.user_id = :user_id
               AND uid.playback_position_ticks > 0
               AND uid.played = 0
             ORDER BY uid.last_played_date DESC
             LIMIT :limit",
        )
        .map_err(|e| Error::database(e.to_string()))?;

    let results = stmt
        .query_map(
            rusqlite::named_params! {
                ":user_id": user_id.to_string(),
                ":limit": limit,
            },
            |row| {
                // Parse item manually since parse_item_row is not public
                let genres_json: String = row.get(16)?;
                let tags_json: String = row.get(17)?;
                let studios_json: String = row.get(18)?;
                let people_json: String = row.get(19)?;
                let provider_ids_json: String = row.get(26)?;

                let item = Item {
                    id: ItemId::from(Uuid::parse_str(&row.get::<_, String>(0)?).unwrap()),
                    library_id: LibraryId::from(
                        Uuid::parse_str(&row.get::<_, String>(1)?).unwrap(),
                    ),
                    parent_id: row
                        .get::<_, Option<String>>(2)?
                        .map(|s| ItemId::from(Uuid::parse_str(&s).unwrap())),
                    item_kind: serde_json::from_str(&format!("\"{}\"", row.get::<_, String>(3)?))
                        .unwrap(),
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
                };

                let user_data = UserItemData {
                    user_id: UserId::from(Uuid::parse_str(&row.get::<_, String>(36)?).unwrap()),
                    item_id: ItemId::from(Uuid::parse_str(&row.get::<_, String>(37)?).unwrap()),
                    playback_position_ticks: row.get(38)?,
                    play_count: row.get(39)?,
                    played: row.get::<_, i32>(40)? != 0,
                    is_favorite: row.get::<_, i32>(41)? != 0,
                    last_played_date: row.get::<_, Option<String>>(42)?.map(|s| {
                        DateTime::parse_from_rfc3339(&s)
                            .unwrap()
                            .with_timezone(&Utc)
                    }),
                };

                Ok((item, user_data))
            },
        )
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(results)
}

/// Get favorite items for a user.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `user_id` - User ID
///
/// # Returns
///
/// * `Ok(Vec<Item>)` - List of favorite items, sorted by name
/// * `Err(Error)` - If a database error occurs
pub fn get_favorites(conn: &Connection, user_id: UserId) -> Result<Vec<Item>> {
    let mut stmt = conn
        .prepare(
            "SELECT i.id, i.library_id, i.parent_id, i.item_kind, i.name, i.sort_name, i.original_title,
                    i.file_path, i.container, i.video_codec, i.audio_codec, i.resolution, i.runtime_ticks,
                    i.size_bytes, i.overview, i.tagline, i.genres, i.tags, i.studios, i.people,
                    i.community_rating, i.critic_rating, i.production_year, i.premiere_date, i.end_date,
                    i.official_rating, i.provider_ids, i.scene_release_name, i.scene_group,
                    i.index_number, i.parent_index_number, i.etag, i.date_created, i.date_modified,
                    i.hdr_type, i.dolby_vision_profile
             FROM items i
             INNER JOIN user_item_data uid ON i.id = uid.item_id
             WHERE uid.user_id = :user_id AND uid.is_favorite = 1
             ORDER BY COALESCE(i.sort_name, i.name) ASC",
        )
        .map_err(|e| Error::database(e.to_string()))?;

    let items = stmt
        .query_map(
            rusqlite::named_params! { ":user_id": user_id.to_string() },
            |row| {
                let genres_json: String = row.get(16)?;
                let tags_json: String = row.get(17)?;
                let studios_json: String = row.get(18)?;
                let people_json: String = row.get(19)?;
                let provider_ids_json: String = row.get(26)?;

                Ok(Item {
                    id: ItemId::from(Uuid::parse_str(&row.get::<_, String>(0)?).unwrap()),
                    library_id: LibraryId::from(
                        Uuid::parse_str(&row.get::<_, String>(1)?).unwrap(),
                    ),
                    parent_id: row
                        .get::<_, Option<String>>(2)?
                        .map(|s| ItemId::from(Uuid::parse_str(&s).unwrap())),
                    item_kind: serde_json::from_str(&format!("\"{}\"", row.get::<_, String>(3)?))
                        .unwrap(),
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
            },
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
    use crate::queries::{items::upsert_item, libraries::create_library, users::create_user};
    use sceneforged_common::{ItemKind, MediaType};

    fn create_test_setup(conn: &Connection) -> (UserId, ItemId) {
        let user = create_user(conn, "testuser", "hash", false).unwrap();
        let library = create_library(conn, "Test", MediaType::Movies, &[]).unwrap();

        let item = Item {
            id: ItemId::new(),
            library_id: library.id,
            parent_id: None,
            item_kind: ItemKind::Movie,
            name: "Test Movie".to_string(),
            sort_name: None,
            original_title: None,
            file_path: Some("/test.mkv".to_string()),
            container: None,
            video_codec: None,
            audio_codec: None,
            resolution: None,
            runtime_ticks: Some(100000000),
            size_bytes: None,
            overview: None,
            tagline: None,
            genres: vec![],
            tags: vec![],
            studios: vec![],
            people: vec![],
            community_rating: None,
            critic_rating: None,
            production_year: None,
            premiere_date: None,
            end_date: None,
            official_rating: None,
            provider_ids: crate::models::ProviderIds::default(),
            scene_release_name: None,
            scene_group: None,
            index_number: None,
            parent_index_number: None,
            etag: None,
            date_created: Utc::now(),
            date_modified: Utc::now(),
            hdr_type: None,
            dolby_vision_profile: None,
        };

        upsert_item(conn, &item).unwrap();
        (user.id, item.id)
    }

    #[test]
    fn test_get_user_item_data_creates_default() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let (user_id, item_id) = create_test_setup(&conn);

        let data = get_user_item_data(&conn, user_id, item_id).unwrap();
        assert_eq!(data.playback_position_ticks, 0);
        assert_eq!(data.play_count, 0);
        assert!(!data.played);
        assert!(!data.is_favorite);
    }

    #[test]
    fn test_update_playback_position() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let (user_id, item_id) = create_test_setup(&conn);

        update_playback_position(&conn, user_id, item_id, 50000000).unwrap();

        let data = get_user_item_data(&conn, user_id, item_id).unwrap();
        assert_eq!(data.playback_position_ticks, 50000000);
        assert!(data.last_played_date.is_some());
    }

    #[test]
    fn test_mark_played() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let (user_id, item_id) = create_test_setup(&conn);

        mark_played(&conn, user_id, item_id).unwrap();

        let data = get_user_item_data(&conn, user_id, item_id).unwrap();
        assert!(data.played);
        assert_eq!(data.play_count, 1);
        assert_eq!(data.playback_position_ticks, 0);
    }

    #[test]
    fn test_mark_unplayed() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let (user_id, item_id) = create_test_setup(&conn);

        mark_played(&conn, user_id, item_id).unwrap();
        mark_unplayed(&conn, user_id, item_id).unwrap();

        let data = get_user_item_data(&conn, user_id, item_id).unwrap();
        assert!(!data.played);
        assert_eq!(data.play_count, 0);
    }

    #[test]
    fn test_toggle_favorite() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let (user_id, item_id) = create_test_setup(&conn);

        let is_fav = toggle_favorite(&conn, user_id, item_id).unwrap();
        assert!(is_fav);

        let data = get_user_item_data(&conn, user_id, item_id).unwrap();
        assert!(data.is_favorite);

        let is_fav = toggle_favorite(&conn, user_id, item_id).unwrap();
        assert!(!is_fav);
    }

    #[test]
    fn test_set_favorite() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let (user_id, item_id) = create_test_setup(&conn);

        set_favorite(&conn, user_id, item_id, true).unwrap();
        let data = get_user_item_data(&conn, user_id, item_id).unwrap();
        assert!(data.is_favorite);

        set_favorite(&conn, user_id, item_id, false).unwrap();
        let data = get_user_item_data(&conn, user_id, item_id).unwrap();
        assert!(!data.is_favorite);
    }

    #[test]
    fn test_get_in_progress_items() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let (user_id, item_id) = create_test_setup(&conn);

        // No in-progress items initially
        let items = get_in_progress_items(&conn, user_id, 10).unwrap();
        assert_eq!(items.len(), 0);

        // Set playback position
        update_playback_position(&conn, user_id, item_id, 50000000).unwrap();

        // Should now have one in-progress item
        let items = get_in_progress_items(&conn, user_id, 10).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].1.playback_position_ticks, 50000000);

        // Mark as played - should no longer be in progress
        mark_played(&conn, user_id, item_id).unwrap();
        let items = get_in_progress_items(&conn, user_id, 10).unwrap();
        assert_eq!(items.len(), 0);
    }

    #[test]
    fn test_get_favorites() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let (user_id, item_id) = create_test_setup(&conn);

        // No favorites initially
        let favorites = get_favorites(&conn, user_id).unwrap();
        assert_eq!(favorites.len(), 0);

        // Add to favorites
        set_favorite(&conn, user_id, item_id, true).unwrap();

        // Should now have one favorite
        let favorites = get_favorites(&conn, user_id).unwrap();
        assert_eq!(favorites.len(), 1);
        assert_eq!(favorites[0].id, item_id);
    }
}
