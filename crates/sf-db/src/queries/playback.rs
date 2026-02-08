//! Playback state operations.

use std::collections::HashMap;

use chrono::Utc;
use rusqlite::Connection;
use sf_core::{Error, ItemId, Result, UserId};

use crate::models::{Item, Playback};

/// Combined playback + favorite data for a single (user, item) pair.
#[derive(Debug, Clone)]
pub struct UserItemData {
    pub position_secs: f64,
    pub completed: bool,
    pub play_count: i32,
    pub is_favorite: bool,
}

const COLS: &str = "user_id, item_id, position_secs, completed, play_count, last_played_at";

/// Insert or update playback state for a (user, item) pair.
pub fn upsert_playback(
    conn: &Connection,
    user_id: UserId,
    item_id: ItemId,
    position_secs: f64,
    completed: bool,
) -> Result<Playback> {
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO playback (user_id, item_id, position_secs, completed, play_count, last_played_at)
         VALUES (?1, ?2, ?3, ?4, 1, ?5)
         ON CONFLICT(user_id, item_id) DO UPDATE SET
            position_secs = excluded.position_secs,
            completed = excluded.completed,
            play_count = play_count + 1,
            last_played_at = excluded.last_played_at",
        rusqlite::params![
            user_id.to_string(),
            item_id.to_string(),
            position_secs,
            completed as i32,
            &now,
        ],
    )
    .map_err(|e| Error::database(e.to_string()))?;

    // Re-read to get the actual play_count value.
    get_playback(conn, user_id, item_id).map(|opt| opt.expect("just upserted"))
}

/// Get playback state for a (user, item) pair.
pub fn get_playback(
    conn: &Connection,
    user_id: UserId,
    item_id: ItemId,
) -> Result<Option<Playback>> {
    let q = format!("SELECT {COLS} FROM playback WHERE user_id = ?1 AND item_id = ?2");
    let result = conn.query_row(
        &q,
        rusqlite::params![user_id.to_string(), item_id.to_string()],
        Playback::from_row,
    );
    match result {
        Ok(p) => Ok(Some(p)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// List recently played items for a user, ordered by `last_played_at DESC`.
pub fn list_recent_playback(
    conn: &Connection,
    user_id: UserId,
    limit: i64,
) -> Result<Vec<Playback>> {
    let q = format!(
        "SELECT {COLS} FROM playback WHERE user_id = ?1
         ORDER BY last_played_at DESC LIMIT ?2"
    );
    let mut stmt = conn.prepare(&q).map_err(|e| Error::database(e.to_string()))?;
    let rows = stmt
        .query_map(
            rusqlite::params![user_id.to_string(), limit],
            Playback::from_row,
        )
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(rows)
}

/// List in-progress items for a user (position > 0, not completed),
/// ordered by `last_played_at DESC`. Used for "Continue Watching".
pub fn list_in_progress(
    conn: &Connection,
    user_id: UserId,
    limit: i64,
) -> Result<Vec<Playback>> {
    let q = format!(
        "SELECT {COLS} FROM playback
         WHERE user_id = ?1 AND position_secs > 0 AND completed = 0
         ORDER BY last_played_at DESC LIMIT ?2"
    );
    let mut stmt = conn.prepare(&q).map_err(|e| Error::database(e.to_string()))?;
    let rows = stmt
        .query_map(
            rusqlite::params![user_id.to_string(), limit],
            Playback::from_row,
        )
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(rows)
}

/// Mark an item as completed (played through).
pub fn mark_played(conn: &Connection, user_id: UserId, item_id: ItemId) -> Result<Playback> {
    upsert_playback(conn, user_id, item_id, 0.0, true)
}

/// Mark an item as unplayed (reset position and completed flag).
pub fn mark_unplayed(conn: &Connection, user_id: UserId, item_id: ItemId) -> Result<bool> {
    let n = conn
        .execute(
            "DELETE FROM playback WHERE user_id = ?1 AND item_id = ?2",
            rusqlite::params![user_id.to_string(), item_id.to_string()],
        )
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

/// Return the "next up" episodes for a user.
///
/// For each series that has in-progress (partially watched, not completed)
/// episodes, finds the *next* unwatched episode after the most recently
/// watched one.  Handles cross-season boundaries.
pub fn next_up(conn: &Connection, user_id: UserId, limit: i64) -> Result<Vec<Item>> {
    // Strategy:
    // 1. Find the latest in-progress episode per series (via playback).
    // 2. For each, find the next episode (same season higher ep, or next season ep 1).
    let sql =
        "WITH watched AS (
            SELECT
                ep.id AS watched_ep_id,
                ep.parent_id AS season_id,
                ep.episode_number AS ep_num,
                season.parent_id AS series_id,
                season.season_number AS season_num,
                p.last_played_at
            FROM playback p
            JOIN items ep ON ep.id = p.item_id AND ep.item_kind = 'episode'
            JOIN items season ON season.id = ep.parent_id AND season.item_kind = 'season'
            WHERE p.user_id = ?1
              AND (p.position_secs > 0 OR p.completed = 1)
        ),
        latest_per_series AS (
            SELECT *,
                ROW_NUMBER() OVER (PARTITION BY series_id ORDER BY last_played_at DESC) AS rn
            FROM watched
        ),
        candidates AS (
            SELECT series_id, season_id, season_num, ep_num, watched_ep_id
            FROM latest_per_series
            WHERE rn = 1
        )
        SELECT next_ep.id, next_ep.library_id, next_ep.item_kind, next_ep.name,
               next_ep.sort_name, next_ep.year, next_ep.overview,
               next_ep.runtime_minutes, next_ep.community_rating,
               next_ep.provider_ids, next_ep.parent_id,
               next_ep.season_number, next_ep.episode_number,
               next_ep.created_at, next_ep.updated_at
        FROM items next_ep
        JOIN items next_season ON next_season.id = next_ep.parent_id AND next_season.item_kind = 'season'
        JOIN candidates c ON next_season.parent_id = c.series_id
        WHERE next_ep.item_kind = 'episode'
          AND (
              (next_season.id = c.season_id AND next_ep.episode_number > c.ep_num)
              OR
              (next_season.season_number > c.season_num)
          )
          AND next_ep.id NOT IN (
              SELECT item_id FROM playback WHERE user_id = ?1 AND completed = 1
          )
        ORDER BY next_season.season_number ASC, next_ep.episode_number ASC
        LIMIT ?2";

    // We want only ONE result per series — deduplicate in application code
    // since SQLite doesn't have DISTINCT ON.
    let mut stmt = conn.prepare(sql).map_err(|e| Error::database(e.to_string()))?;
    let rows = stmt
        .query_map(
            rusqlite::params![user_id.to_string(), limit * 5],
            Item::from_row,
        )
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;

    // Deduplicate: one next-up per series (first = lowest season/ep = correct).
    let mut seen_series = std::collections::HashSet::new();
    let mut result = Vec::new();
    for item in rows {
        // The series_id is the grandparent (season's parent_id).
        // We can derive it from the season's parent, but we already have parent_id (the season).
        // Look up the season's parent_id as series_id.
        let season_id = item.parent_id;
        if let Some(sid) = season_id {
            // Get the series id from the season.
            let series_id: Option<String> = conn
                .query_row(
                    "SELECT parent_id FROM items WHERE id = ?1",
                    [sid.to_string()],
                    |r| r.get(0),
                )
                .ok()
                .flatten();
            let key = series_id.unwrap_or_else(|| sid.to_string());
            if seen_series.insert(key) {
                result.push(item);
                if result.len() >= limit as usize {
                    break;
                }
            }
        } else {
            result.push(item);
            if result.len() >= limit as usize {
                break;
            }
        }
    }

    Ok(result)
}

/// Batch-fetch playback + favorite data for a list of item IDs.
///
/// Runs 2 queries (playback + favorites) instead of N+1, then merges results.
pub fn batch_get_user_data(
    conn: &Connection,
    user_id: UserId,
    item_ids: &[ItemId],
) -> Result<HashMap<ItemId, UserItemData>> {
    if item_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut result: HashMap<ItemId, UserItemData> = HashMap::new();

    // Build IN clause placeholders.
    let placeholders: Vec<String> = (0..item_ids.len()).map(|i| format!("?{}", i + 2)).collect();
    let in_clause = placeholders.join(",");

    // Query 1: playback data
    let sql = format!(
        "SELECT item_id, position_secs, completed, play_count FROM playback WHERE user_id = ?1 AND item_id IN ({in_clause})"
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    params.push(Box::new(user_id.to_string()));
    for id in item_ids {
        params.push(Box::new(id.to_string()));
    }
    let params_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql).map_err(|e| Error::database(e.to_string()))?;
    let rows = stmt
        .query_map(params_refs.as_slice(), |row| {
            let item_id_str: String = row.get(0)?;
            let position_secs: f64 = row.get::<_, f64>(1).unwrap_or(0.0);
            let completed: bool = row.get::<_, i32>(2).unwrap_or(0) != 0;
            let play_count: i32 = row.get::<_, i32>(3).unwrap_or(0);
            Ok((item_id_str, position_secs, completed, play_count))
        })
        .map_err(|e| Error::database(e.to_string()))?;

    for row in rows {
        let (id_str, position_secs, completed, play_count) =
            row.map_err(|e| Error::database(e.to_string()))?;
        if let Ok(item_id) = id_str.parse::<ItemId>() {
            result.insert(
                item_id,
                UserItemData {
                    position_secs,
                    completed,
                    play_count,
                    is_favorite: false,
                },
            );
        }
    }

    // Query 2: favorites
    let sql2 = format!(
        "SELECT item_id FROM favorites WHERE user_id = ?1 AND item_id IN ({in_clause})"
    );
    let mut stmt2 = conn.prepare(&sql2).map_err(|e| Error::database(e.to_string()))?;
    let fav_rows = stmt2
        .query_map(params_refs.as_slice(), |row| {
            let item_id_str: String = row.get(0)?;
            Ok(item_id_str)
        })
        .map_err(|e| Error::database(e.to_string()))?;

    for row in fav_rows {
        let id_str = row.map_err(|e| Error::database(e.to_string()))?;
        if let Ok(item_id) = id_str.parse::<ItemId>() {
            result
                .entry(item_id)
                .and_modify(|d| d.is_favorite = true)
                .or_insert(UserItemData {
                    position_secs: 0.0,
                    completed: false,
                    play_count: 0,
                    is_favorite: true,
                });
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_memory_pool;
    use crate::queries::{items, libraries, users};

    fn setup() -> (
        r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>,
        UserId,
        ItemId,
    ) {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let user = users::create_user(&conn, "player", "h", "user").unwrap();
        let lib = libraries::create_library(&conn, "M", "movies", &[], &serde_json::json!({})).unwrap();
        let item = items::create_item(
            &conn, lib.id, "movie", "T", None, None, None, None, None, None, None, None, None,
        )
        .unwrap();
        (conn, user.id, item.id)
    }

    #[test]
    fn upsert_and_get() {
        let (conn, uid, iid) = setup();
        let pb = upsert_playback(&conn, uid, iid, 120.0, false).unwrap();
        assert!((pb.position_secs - 120.0).abs() < f64::EPSILON);
        assert!(!pb.completed);
        assert_eq!(pb.play_count, 1);

        // update
        let pb2 = upsert_playback(&conn, uid, iid, 240.0, true).unwrap();
        assert_eq!(pb2.play_count, 2);
        assert!(pb2.completed);
    }

    #[test]
    fn list_recent() {
        let (conn, uid, iid) = setup();
        upsert_playback(&conn, uid, iid, 10.0, false).unwrap();
        let list = list_recent_playback(&conn, uid, 10).unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn next_up_basic() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let user = users::create_user(&conn, "viewer", "h", "user").unwrap();
        let lib =
            libraries::create_library(&conn, "TV", "tvshows", &[], &serde_json::json!({}))
                .unwrap();

        // Create series -> season -> 3 episodes.
        let series = items::create_item(
            &conn, lib.id, "series", "Breaking Bad", None, Some(2008), None, None, None,
            None, None, None, None,
        )
        .unwrap();
        let season = items::create_item(
            &conn, lib.id, "season", "Season 1", None, None, None, None, None, None,
            Some(series.id), Some(1), None,
        )
        .unwrap();
        let ep1 = items::create_item(
            &conn, lib.id, "episode", "Ep 1", None, None, None, None, None, None,
            Some(season.id), Some(1), Some(1),
        )
        .unwrap();
        let ep2 = items::create_item(
            &conn, lib.id, "episode", "Ep 2", None, None, None, None, None, None,
            Some(season.id), Some(1), Some(2),
        )
        .unwrap();
        let _ep3 = items::create_item(
            &conn, lib.id, "episode", "Ep 3", None, None, None, None, None, None,
            Some(season.id), Some(1), Some(3),
        )
        .unwrap();

        // User watched episode 1 partially.
        upsert_playback(&conn, user.id, ep1.id, 300.0, false).unwrap();

        let results = next_up(&conn, user.id, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, ep2.id, "should suggest episode 2 after watching ep 1");
    }
}
