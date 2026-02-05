//! Playback state operations.

use chrono::Utc;
use rusqlite::Connection;
use sf_core::{Error, ItemId, Result, UserId};

use crate::models::Playback;

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
}
