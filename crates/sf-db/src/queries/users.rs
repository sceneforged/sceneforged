//! User CRUD operations.

use chrono::Utc;
use rusqlite::Connection;
use sf_core::{Error, Result, UserId};

use crate::models::User;

/// Create a new user and return it.
pub fn create_user(
    conn: &Connection,
    username: &str,
    password_hash: &str,
    role: &str,
) -> Result<User> {
    let id = UserId::new();
    let created_at = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO users (id, username, password_hash, role, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id.to_string(), username, password_hash, role, created_at],
    )
    .map_err(|e| {
        if e.to_string().contains("UNIQUE constraint failed") {
            Error::Conflict(format!("Username '{username}' already exists"))
        } else {
            Error::database(e.to_string())
        }
    })?;

    Ok(User {
        id,
        username: username.to_string(),
        password_hash: password_hash.to_string(),
        role: role.to_string(),
        created_at,
    })
}

/// Get a user by primary key.
pub fn get_user_by_id(conn: &Connection, id: UserId) -> Result<Option<User>> {
    let result = conn.query_row(
        "SELECT id, username, password_hash, role, created_at FROM users WHERE id = ?1",
        [id.to_string()],
        User::from_row,
    );
    match result {
        Ok(u) => Ok(Some(u)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Get a user by username.
pub fn get_user_by_username(conn: &Connection, username: &str) -> Result<Option<User>> {
    let result = conn.query_row(
        "SELECT id, username, password_hash, role, created_at FROM users WHERE username = ?1",
        [username],
        User::from_row,
    );
    match result {
        Ok(u) => Ok(Some(u)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Delete a user by ID. Returns true if a row was deleted.
pub fn delete_user(conn: &Connection, id: UserId) -> Result<bool> {
    let n = conn
        .execute("DELETE FROM users WHERE id = ?1", [id.to_string()])
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_memory_pool;

    #[test]
    fn create_and_get() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let u = create_user(&conn, "alice", "hash", "admin").unwrap();
        assert_eq!(u.username, "alice");

        let found = get_user_by_id(&conn, u.id).unwrap().unwrap();
        assert_eq!(found.username, "alice");
    }

    #[test]
    fn get_by_username() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        create_user(&conn, "bob", "hash", "user").unwrap();
        let found = get_user_by_username(&conn, "bob").unwrap();
        assert!(found.is_some());
    }

    #[test]
    fn duplicate_username() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        create_user(&conn, "dup", "h1", "user").unwrap();
        assert!(create_user(&conn, "dup", "h2", "user").is_err());
    }

    #[test]
    fn delete() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let u = create_user(&conn, "del", "h", "user").unwrap();
        assert!(delete_user(&conn, u.id).unwrap());
        assert!(get_user_by_id(&conn, u.id).unwrap().is_none());
    }
}
