//! Authentication token operations.

use rusqlite::Connection;
use sf_core::{Error, Result, SessionId, UserId};

use crate::models::AuthToken;

const COLS: &str = "id, user_id, token, expires_at";

/// Create a new auth token.
pub fn create_token(
    conn: &Connection,
    user_id: UserId,
    token: &str,
    expires_at: &str,
) -> Result<AuthToken> {
    let id = SessionId::new();

    conn.execute(
        "INSERT INTO auth_tokens (id, user_id, token, expires_at) VALUES (?1,?2,?3,?4)",
        rusqlite::params![id.to_string(), user_id.to_string(), token, expires_at],
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(AuthToken {
        id,
        user_id,
        token: token.to_string(),
        expires_at: expires_at.to_string(),
    })
}

/// Look up a token by its value.
pub fn get_token(conn: &Connection, token: &str) -> Result<Option<AuthToken>> {
    let q = format!("SELECT {COLS} FROM auth_tokens WHERE token = ?1");
    let result = conn.query_row(&q, [token], AuthToken::from_row);
    match result {
        Ok(t) => Ok(Some(t)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Delete a specific token by value.
pub fn delete_token(conn: &Connection, token: &str) -> Result<bool> {
    let n = conn
        .execute("DELETE FROM auth_tokens WHERE token = ?1", [token])
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

/// Delete all tokens whose `expires_at` is in the past.
pub fn delete_expired_tokens(conn: &Connection, now: &str) -> Result<usize> {
    let n = conn
        .execute(
            "DELETE FROM auth_tokens WHERE expires_at < ?1",
            [now],
        )
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_memory_pool;
    use crate::queries::users;

    #[test]
    fn create_get_delete() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let user = users::create_user(&conn, "tok_user", "hash", "user").unwrap();

        let tok = create_token(&conn, user.id, "abc123", "2099-01-01T00:00:00Z").unwrap();
        assert_eq!(tok.token, "abc123");

        let found = get_token(&conn, "abc123").unwrap().unwrap();
        assert_eq!(found.user_id, user.id);

        assert!(delete_token(&conn, "abc123").unwrap());
        assert!(get_token(&conn, "abc123").unwrap().is_none());
    }

    #[test]
    fn delete_expired() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let user = users::create_user(&conn, "exp_user", "hash", "user").unwrap();

        create_token(&conn, user.id, "old", "2000-01-01T00:00:00Z").unwrap();
        create_token(&conn, user.id, "new", "2099-01-01T00:00:00Z").unwrap();

        let deleted = delete_expired_tokens(&conn, "2025-06-01T00:00:00Z").unwrap();
        assert_eq!(deleted, 1);

        // "new" should still exist
        assert!(get_token(&conn, "new").unwrap().is_some());
    }
}
