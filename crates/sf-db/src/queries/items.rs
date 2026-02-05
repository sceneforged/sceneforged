//! Item CRUD, list, and search operations.

use chrono::Utc;
use rusqlite::Connection;
use sf_core::{Error, ItemId, LibraryId, Result};

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

/// Search items by name (LIKE '%query%').
pub fn search_items(conn: &Connection, query: &str, limit: i64) -> Result<Vec<Item>> {
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
}
