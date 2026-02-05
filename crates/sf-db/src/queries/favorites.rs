//! Favorite operations.

use chrono::Utc;
use rusqlite::Connection;
use sf_core::{Error, ItemId, Result, UserId};

use crate::models::Favorite;

const COLS: &str = "user_id, item_id, created_at";

/// Add an item to a user's favorites. No-op if already favorited.
pub fn add_favorite(conn: &Connection, user_id: UserId, item_id: ItemId) -> Result<Favorite> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR IGNORE INTO favorites (user_id, item_id, created_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![user_id.to_string(), item_id.to_string(), &now],
    )
    .map_err(|e| Error::database(e.to_string()))?;

    // Re-read to get the actual created_at (may differ on ignore).
    get_favorite(conn, user_id, item_id).map(|opt| opt.expect("just inserted or already exists"))
}

/// Remove an item from a user's favorites. Returns true if removed.
pub fn remove_favorite(conn: &Connection, user_id: UserId, item_id: ItemId) -> Result<bool> {
    let n = conn
        .execute(
            "DELETE FROM favorites WHERE user_id = ?1 AND item_id = ?2",
            rusqlite::params![user_id.to_string(), item_id.to_string()],
        )
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

/// Check if an item is in a user's favorites.
pub fn get_favorite(
    conn: &Connection,
    user_id: UserId,
    item_id: ItemId,
) -> Result<Option<Favorite>> {
    let q = format!("SELECT {COLS} FROM favorites WHERE user_id = ?1 AND item_id = ?2");
    let result = conn.query_row(
        &q,
        rusqlite::params![user_id.to_string(), item_id.to_string()],
        Favorite::from_row,
    );
    match result {
        Ok(f) => Ok(Some(f)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// List a user's favorites ordered by most recently added.
pub fn list_favorites(
    conn: &Connection,
    user_id: UserId,
    limit: i64,
) -> Result<Vec<Favorite>> {
    let q = format!(
        "SELECT {COLS} FROM favorites WHERE user_id = ?1 ORDER BY created_at DESC LIMIT ?2"
    );
    let mut stmt = conn.prepare(&q).map_err(|e| Error::database(e.to_string()))?;
    let rows = stmt
        .query_map(
            rusqlite::params![user_id.to_string(), limit],
            Favorite::from_row,
        )
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(rows)
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
        let user = users::create_user(&conn, "favuser", "h", "user").unwrap();
        let lib =
            libraries::create_library(&conn, "M", "movies", &[], &serde_json::json!({})).unwrap();
        let item = items::create_item(
            &conn, lib.id, "movie", "T", None, None, None, None, None, None, None, None, None,
        )
        .unwrap();
        (conn, user.id, item.id)
    }

    #[test]
    fn add_and_remove() {
        let (conn, uid, iid) = setup();
        let fav = add_favorite(&conn, uid, iid).unwrap();
        assert_eq!(fav.item_id, iid);

        // Duplicate add is a no-op.
        let fav2 = add_favorite(&conn, uid, iid).unwrap();
        assert_eq!(fav2.item_id, iid);

        assert!(remove_favorite(&conn, uid, iid).unwrap());
        assert!(get_favorite(&conn, uid, iid).unwrap().is_none());
    }

    #[test]
    fn list() {
        let (conn, uid, iid) = setup();
        add_favorite(&conn, uid, iid).unwrap();
        let list = list_favorites(&conn, uid, 50).unwrap();
        assert_eq!(list.len(), 1);
    }
}
