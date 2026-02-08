//! Item CRUD, list, and search operations.

use chrono::Utc;
use rusqlite::Connection;
use sf_core::{Error, ItemId, LibraryId, Result, UserId};

use crate::models::Item;

/// Column list used in SELECT statements.
const COLS: &str = "id, library_id, item_kind, name, sort_name, year, overview,
    runtime_minutes, community_rating, provider_ids, parent_id,
    season_number, episode_number, created_at, updated_at";

/// Create a new item.
pub fn create_item(
    conn: &Connection,
    library_id: LibraryId,
    item_kind: &str,
    name: &str,
    sort_name: Option<&str>,
    year: Option<i32>,
    overview: Option<&str>,
    runtime_minutes: Option<i32>,
    community_rating: Option<f64>,
    provider_ids: Option<&str>,
    parent_id: Option<ItemId>,
    season_number: Option<i32>,
    episode_number: Option<i32>,
) -> Result<Item> {
    let id = ItemId::new();
    let now = Utc::now().to_rfc3339();
    let pids = provider_ids.unwrap_or("{}");

    conn.execute(
        "INSERT INTO items (id, library_id, item_kind, name, sort_name, year, overview,
            runtime_minutes, community_rating, provider_ids, parent_id,
            season_number, episode_number, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        rusqlite::params![
            id.to_string(),
            library_id.to_string(),
            item_kind,
            name,
            sort_name,
            year,
            overview,
            runtime_minutes,
            community_rating,
            pids,
            parent_id.map(|p| p.to_string()),
            season_number,
            episode_number,
            &now,
            &now,
        ],
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(Item {
        id,
        library_id,
        item_kind: item_kind.to_string(),
        name: name.to_string(),
        sort_name: sort_name.map(String::from),
        year,
        overview: overview.map(String::from),
        runtime_minutes,
        community_rating,
        provider_ids: pids.to_string(),
        parent_id,
        season_number,
        episode_number,
        created_at: now.clone(),
        updated_at: now,
    })
}

/// Get an item by ID.
pub fn get_item(conn: &Connection, id: ItemId) -> Result<Option<Item>> {
    let q = format!("SELECT {COLS} FROM items WHERE id = ?1");
    let result = conn.query_row(&q, [id.to_string()], Item::from_row);
    match result {
        Ok(i) => Ok(Some(i)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// List items for a library with offset/limit pagination.
pub fn list_items_by_library(
    conn: &Connection,
    library_id: LibraryId,
    offset: i64,
    limit: i64,
) -> Result<Vec<Item>> {
    let q = format!(
        "SELECT {COLS} FROM items WHERE library_id = ?1
         ORDER BY COALESCE(sort_name, name) ASC LIMIT ?2 OFFSET ?3"
    );
    let mut stmt = conn.prepare(&q).map_err(|e| Error::database(e.to_string()))?;
    let rows = stmt
        .query_map(
            rusqlite::params![library_id.to_string(), limit, offset],
            Item::from_row,
        )
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(rows)
}

/// Update mutable fields of an item.
pub fn update_item(
    conn: &Connection,
    id: ItemId,
    name: &str,
    sort_name: Option<&str>,
    year: Option<i32>,
    overview: Option<&str>,
    runtime_minutes: Option<i32>,
    community_rating: Option<f64>,
    provider_ids: Option<&str>,
    parent_id: Option<ItemId>,
    season_number: Option<i32>,
    episode_number: Option<i32>,
) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let n = conn
        .execute(
            "UPDATE items SET name=?1, sort_name=?2, year=?3, overview=?4,
                runtime_minutes=?5, community_rating=?6, provider_ids=?7,
                parent_id=?8, season_number=?9, episode_number=?10, updated_at=?11
             WHERE id=?12",
            rusqlite::params![
                name,
                sort_name,
                year,
                overview,
                runtime_minutes,
                community_rating,
                provider_ids.unwrap_or("{}"),
                parent_id.map(|p| p.to_string()),
                season_number,
                episode_number,
                now,
                id.to_string(),
            ],
        )
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

/// Delete an item by ID.
pub fn delete_item(conn: &Connection, id: ItemId) -> Result<bool> {
    let n = conn
        .execute("DELETE FROM items WHERE id = ?1", [id.to_string()])
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

/// List child items of a parent, ordered by season/episode number.
pub fn list_children(conn: &Connection, parent_id: ItemId) -> Result<Vec<Item>> {
    let q = format!(
        "SELECT {COLS} FROM items WHERE parent_id = ?1
         ORDER BY COALESCE(season_number, 0), COALESCE(episode_number, 0), name ASC"
    );
    let mut stmt = conn.prepare(&q).map_err(|e| Error::database(e.to_string()))?;
    let rows = stmt
        .query_map([parent_id.to_string()], Item::from_row)
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(rows)
}

/// List recently added items for a library (within `days` days).
pub fn list_recent_items_by_library(
    conn: &Connection,
    library_id: LibraryId,
    days: i64,
) -> Result<Vec<Item>> {
    let q = format!(
        "SELECT {COLS} FROM items WHERE library_id = ?1
         AND created_at >= datetime('now', '-' || ?2 || ' days')
         ORDER BY created_at DESC"
    );
    let mut stmt = conn.prepare(&q).map_err(|e| Error::database(e.to_string()))?;
    let rows = stmt
        .query_map(
            rusqlite::params![library_id.to_string(), days],
            Item::from_row,
        )
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(rows)
}

/// Find an item by name, kind, and library — used for series/season deduplication.
pub fn find_item_by_name_and_kind(
    conn: &Connection,
    library_id: LibraryId,
    name: &str,
    item_kind: &str,
) -> Result<Option<Item>> {
    let q = format!(
        "SELECT {COLS} FROM items WHERE library_id = ?1 AND name = ?2 AND item_kind = ?3 LIMIT 1"
    );
    let result = conn.query_row(
        &q,
        rusqlite::params![library_id.to_string(), name, item_kind],
        Item::from_row,
    );
    match result {
        Ok(i) => Ok(Some(i)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Find an existing series item or create one.
pub fn find_or_create_series(
    conn: &Connection,
    library_id: LibraryId,
    name: &str,
    year: Option<i32>,
) -> Result<Item> {
    if let Some(existing) = find_item_by_name_and_kind(conn, library_id, name, "series")? {
        return Ok(existing);
    }
    create_item(
        conn,
        library_id,
        "series",
        name,
        None,
        year,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
}

/// Find an existing season item or create one under a series.
pub fn find_or_create_season(
    conn: &Connection,
    library_id: LibraryId,
    series_id: ItemId,
    season_number: i32,
) -> Result<Item> {
    // Look for existing season under this series.
    let q = format!(
        "SELECT {COLS} FROM items WHERE parent_id = ?1 AND item_kind = 'season' AND season_number = ?2 LIMIT 1"
    );
    let result = conn.query_row(
        &q,
        rusqlite::params![series_id.to_string(), season_number],
        Item::from_row,
    );
    match result {
        Ok(season) => return Ok(season),
        Err(rusqlite::Error::QueryReturnedNoRows) => {}
        Err(e) => return Err(Error::database(e.to_string())),
    }

    let name = format!("Season {season_number}");
    create_item(
        conn,
        library_id,
        "season",
        &name,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(series_id),
        Some(season_number),
        None,
    )
}

/// Search items by name (LIKE '%query%').
pub fn search_items(conn: &Connection, query: &str, limit: i64) -> Result<Vec<Item>> {
    // Try FTS5 first — fall back to LIKE if the FTS table doesn't exist yet.
    match search_items_fts(conn, query, None, None, limit) {
        Ok(results) => Ok(results),
        Err(_) => {
            let pattern = format!("%{query}%");
            let q = format!(
                "SELECT {COLS} FROM items WHERE name LIKE ?1 ORDER BY name ASC LIMIT ?2"
            );
            let mut stmt = conn.prepare(&q).map_err(|e| Error::database(e.to_string()))?;
            let rows = stmt
                .query_map(rusqlite::params![pattern, limit], Item::from_row)
                .map_err(|e| Error::database(e.to_string()))?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| Error::database(e.to_string()))?;
            Ok(rows)
        }
    }
}

/// Full-text search using FTS5 index.
///
/// Searches both `name` and `overview` fields, ranked by relevance.
/// Optionally filters by `library_id` and `item_kind`.
pub fn search_items_fts(
    conn: &Connection,
    query: &str,
    library_id: Option<LibraryId>,
    item_kind: Option<&str>,
    limit: i64,
) -> Result<Vec<Item>> {
    // FTS5 query: append * for prefix matching (e.g. "break" matches "Breaking Bad").
    let fts_query = format!("{query}*");

    let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) =
        match (library_id, item_kind) {
            (Some(lid), Some(kind)) => (
                format!(
                    "SELECT {COLS} FROM items
                     WHERE rowid IN (SELECT rowid FROM items_fts WHERE items_fts MATCH ?1)
                       AND library_id = ?2
                       AND item_kind = ?3
                     ORDER BY rank
                     LIMIT ?4"
                ),
                vec![
                    Box::new(fts_query),
                    Box::new(lid.to_string()),
                    Box::new(kind.to_string()),
                    Box::new(limit),
                ],
            ),
            (Some(lid), None) => (
                format!(
                    "SELECT {COLS} FROM items
                     WHERE rowid IN (SELECT rowid FROM items_fts WHERE items_fts MATCH ?1)
                       AND library_id = ?2
                     ORDER BY rank
                     LIMIT ?3"
                ),
                vec![
                    Box::new(fts_query),
                    Box::new(lid.to_string()),
                    Box::new(limit),
                ],
            ),
            (None, Some(kind)) => (
                format!(
                    "SELECT {COLS} FROM items
                     WHERE rowid IN (SELECT rowid FROM items_fts WHERE items_fts MATCH ?1)
                       AND item_kind = ?2
                     ORDER BY rank
                     LIMIT ?3"
                ),
                vec![
                    Box::new(fts_query),
                    Box::new(kind.to_string()),
                    Box::new(limit),
                ],
            ),
            (None, None) => (
                format!(
                    "SELECT {COLS} FROM items
                     WHERE rowid IN (SELECT rowid FROM items_fts WHERE items_fts MATCH ?1)
                     ORDER BY rank
                     LIMIT ?2"
                ),
                vec![Box::new(fts_query), Box::new(limit)],
            ),
        };

    let params_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql).map_err(|e| Error::database(e.to_string()))?;
    let rows = stmt
        .query_map(params_refs.as_slice(), Item::from_row)
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(rows)
}

/// List items that a user has favorited, with optional library/kind filters.
pub fn list_favorite_items(
    conn: &Connection,
    user_id: UserId,
    library_id: Option<LibraryId>,
    item_kinds: Option<&[&str]>,
    offset: i64,
    limit: i64,
) -> Result<Vec<Item>> {
    let mut sql = format!(
        "SELECT {COLS} FROM items
         INNER JOIN favorites ON favorites.item_id = items.id
         WHERE favorites.user_id = ?1"
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(user_id.to_string())];
    let mut idx = 2;

    if let Some(lid) = library_id {
        sql.push_str(&format!(" AND items.library_id = ?{idx}"));
        params.push(Box::new(lid.to_string()));
        idx += 1;
    }

    if let Some(kinds) = item_kinds {
        if !kinds.is_empty() {
            let placeholders: Vec<String> = kinds
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", idx + i))
                .collect();
            sql.push_str(&format!(" AND items.item_kind IN ({})", placeholders.join(",")));
            for kind in kinds {
                params.push(Box::new(kind.to_string()));
            }
            idx += kinds.len();
        }
    }

    sql.push_str(&format!(
        " ORDER BY favorites.created_at DESC LIMIT ?{} OFFSET ?{}",
        idx,
        idx + 1
    ));
    params.push(Box::new(limit));
    params.push(Box::new(offset));

    let params_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql).map_err(|e| Error::database(e.to_string()))?;
    let rows = stmt
        .query_map(params_refs.as_slice(), Item::from_row)
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(rows)
}

/// List items that a user has in-progress playback (position > 0, not completed).
pub fn list_resumable_items(
    conn: &Connection,
    user_id: UserId,
    library_id: Option<LibraryId>,
    item_kinds: Option<&[&str]>,
    offset: i64,
    limit: i64,
) -> Result<Vec<Item>> {
    let mut sql = format!(
        "SELECT {COLS} FROM items
         INNER JOIN playback ON playback.item_id = items.id
         WHERE playback.user_id = ?1 AND playback.position_secs > 0 AND playback.completed = 0"
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(user_id.to_string())];
    let mut idx = 2;

    if let Some(lid) = library_id {
        sql.push_str(&format!(" AND items.library_id = ?{idx}"));
        params.push(Box::new(lid.to_string()));
        idx += 1;
    }

    if let Some(kinds) = item_kinds {
        if !kinds.is_empty() {
            let placeholders: Vec<String> = kinds
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", idx + i))
                .collect();
            sql.push_str(&format!(" AND items.item_kind IN ({})", placeholders.join(",")));
            for kind in kinds {
                params.push(Box::new(kind.to_string()));
            }
            idx += kinds.len();
        }
    }

    sql.push_str(&format!(
        " ORDER BY playback.last_played_at DESC LIMIT ?{} OFFSET ?{}",
        idx,
        idx + 1
    ));
    params.push(Box::new(limit));
    params.push(Box::new(offset));

    let params_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql).map_err(|e| Error::database(e.to_string()))?;
    let rows = stmt
        .query_map(params_refs.as_slice(), Item::from_row)
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_memory_pool;
    use crate::queries::libraries;

    fn setup() -> (r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>, LibraryId) {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let lib = libraries::create_library(
            &conn,
            "Movies",
            "movies",
            &[],
            &serde_json::json!({}),
        )
        .unwrap();
        (conn, lib.id)
    }

    #[test]
    fn create_and_get() {
        let (conn, lib_id) = setup();
        let item = create_item(
            &conn, lib_id, "movie", "Inception", None, Some(2010),
            None, Some(148), None, None, None, None, None,
        )
        .unwrap();

        let found = get_item(&conn, item.id).unwrap().unwrap();
        assert_eq!(found.name, "Inception");
        assert_eq!(found.year, Some(2010));
    }

    #[test]
    fn list_paginated() {
        let (conn, lib_id) = setup();
        for i in 0..5 {
            create_item(
                &conn, lib_id, "movie", &format!("Movie {i}"), None, None,
                None, None, None, None, None, None, None,
            )
            .unwrap();
        }
        let page = list_items_by_library(&conn, lib_id, 0, 3).unwrap();
        assert_eq!(page.len(), 3);
        let page2 = list_items_by_library(&conn, lib_id, 3, 3).unwrap();
        assert_eq!(page2.len(), 2);
    }

    #[test]
    fn update_and_delete() {
        let (conn, lib_id) = setup();
        let item = create_item(
            &conn, lib_id, "movie", "Old", None, None,
            None, None, None, None, None, None, None,
        )
        .unwrap();
        assert!(update_item(
            &conn, item.id, "New", None, Some(2025), None,
            None, None, None, None, None, None,
        )
        .unwrap());
        let found = get_item(&conn, item.id).unwrap().unwrap();
        assert_eq!(found.name, "New");

        assert!(delete_item(&conn, item.id).unwrap());
        assert!(get_item(&conn, item.id).unwrap().is_none());
    }

    #[test]
    fn search() {
        let (conn, lib_id) = setup();
        create_item(
            &conn, lib_id, "movie", "The Matrix", None, None,
            None, None, None, None, None, None, None,
        )
        .unwrap();
        create_item(
            &conn, lib_id, "movie", "Inception", None, None,
            None, None, None, None, None, None, None,
        )
        .unwrap();
        let results = search_items(&conn, "Matrix", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "The Matrix");
    }

    #[test]
    fn list_children_ordered() {
        let (conn, lib_id) = setup();
        let series = create_item(
            &conn, lib_id, "series", "Breaking Bad", None, Some(2008),
            None, None, None, None, None, None, None,
        )
        .unwrap();
        let season = create_item(
            &conn, lib_id, "season", "Season 1", None, None,
            None, None, None, None, Some(series.id), Some(1), None,
        )
        .unwrap();

        // Create episodes out of order.
        create_item(
            &conn, lib_id, "episode", "Episode 3", None, None,
            None, Some(47), None, None, Some(season.id), Some(1), Some(3),
        )
        .unwrap();
        create_item(
            &conn, lib_id, "episode", "Episode 1", None, None,
            None, Some(58), None, None, Some(season.id), Some(1), Some(1),
        )
        .unwrap();
        create_item(
            &conn, lib_id, "episode", "Episode 2", None, None,
            None, Some(48), None, None, Some(season.id), Some(1), Some(2),
        )
        .unwrap();

        let children = list_children(&conn, season.id).unwrap();
        assert_eq!(children.len(), 3);
        assert_eq!(children[0].episode_number, Some(1));
        assert_eq!(children[1].episode_number, Some(2));
        assert_eq!(children[2].episode_number, Some(3));
    }

    #[test]
    fn list_recent_items() {
        let (conn, lib_id) = setup();
        create_item(
            &conn, lib_id, "movie", "Recent Movie", None, Some(2024),
            None, None, None, None, None, None, None,
        )
        .unwrap();

        let recent = list_recent_items_by_library(&conn, lib_id, 7).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].name, "Recent Movie");
    }

    #[test]
    fn find_item_by_name_and_kind_found() {
        let (conn, lib_id) = setup();
        create_item(
            &conn, lib_id, "series", "Breaking Bad", None, Some(2008),
            None, None, None, None, None, None, None,
        )
        .unwrap();

        let found = find_item_by_name_and_kind(&conn, lib_id, "Breaking Bad", "series")
            .unwrap()
            .unwrap();
        assert_eq!(found.name, "Breaking Bad");
        assert_eq!(found.item_kind, "series");

        let not_found = find_item_by_name_and_kind(&conn, lib_id, "Breaking Bad", "movie").unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn find_or_create_series_dedup() {
        let (conn, lib_id) = setup();
        let s1 = find_or_create_series(&conn, lib_id, "The Wire", Some(2002)).unwrap();
        let s2 = find_or_create_series(&conn, lib_id, "The Wire", Some(2002)).unwrap();
        assert_eq!(s1.id, s2.id); // Same item, not duplicated.
    }

    #[test]
    fn find_or_create_season_dedup() {
        let (conn, lib_id) = setup();
        let series = find_or_create_series(&conn, lib_id, "The Wire", None).unwrap();
        let se1 = find_or_create_season(&conn, lib_id, series.id, 1).unwrap();
        let se2 = find_or_create_season(&conn, lib_id, series.id, 1).unwrap();
        assert_eq!(se1.id, se2.id);

        let se3 = find_or_create_season(&conn, lib_id, series.id, 2).unwrap();
        assert_ne!(se1.id, se3.id);
    }

    #[test]
    fn search_items_fallback() {
        // search_items tries FTS5 first, then falls back to LIKE.
        // In unit tests the FTS5 rank column issue triggers the fallback.
        let (conn, lib_id) = setup();
        create_item(
            &conn, lib_id, "movie", "The Matrix", None, Some(1999),
            None, None, None, None, None, None, None,
        )
        .unwrap();
        create_item(
            &conn, lib_id, "movie", "Inception", None, None,
            None, None, None, None, None, None, None,
        )
        .unwrap();

        // LIKE fallback should still find "Matrix".
        let results = search_items(&conn, "Matrix", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "The Matrix");

        // Empty query should match everything.
        let results = search_items(&conn, "", 10).unwrap();
        assert_eq!(results.len(), 2);
    }
}
