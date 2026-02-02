//! User database queries.
//!
//! This module provides CRUD operations for user accounts.

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use sceneforged_common::{Error, Result, UserId};
use uuid::Uuid;

use crate::models::User;

/// Create a new user.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `username` - Unique username
/// * `password_hash` - Hashed password
/// * `is_admin` - Whether the user has admin privileges
///
/// # Returns
///
/// * `Ok(User)` - The created user
/// * `Err(Error)` - If the username already exists or database error occurs
pub fn create_user(
    conn: &Connection,
    username: &str,
    password_hash: &str,
    is_admin: bool,
) -> Result<User> {
    let id = UserId::new();
    let created_at = Utc::now();

    conn.execute(
        "INSERT INTO users (id, username, password_hash, is_admin, created_at)
         VALUES (:id, :username, :password_hash, :is_admin, :created_at)",
        rusqlite::named_params! {
            ":id": id.to_string(),
            ":username": username,
            ":password_hash": password_hash,
            ":is_admin": is_admin,
            ":created_at": created_at.to_rfc3339(),
        },
    )
    .map_err(|e| {
        if e.to_string().contains("UNIQUE constraint failed") {
            Error::InvalidInput(format!("Username '{}' already exists", username))
        } else {
            Error::database(e.to_string())
        }
    })?;

    Ok(User {
        id,
        username: username.to_string(),
        password_hash: password_hash.to_string(),
        is_admin,
        created_at,
    })
}

/// Get a user by ID.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `id` - User ID
///
/// # Returns
///
/// * `Ok(Some(User))` - The user if found
/// * `Ok(None)` - If the user does not exist
/// * `Err(Error)` - If a database error occurs
pub fn get_user(conn: &Connection, id: UserId) -> Result<Option<User>> {
    let result = conn.query_row(
        "SELECT id, username, password_hash, is_admin, created_at
         FROM users WHERE id = :id",
        rusqlite::named_params! { ":id": id.to_string() },
        |row| {
            Ok(User {
                id: UserId::from(Uuid::parse_str(&row.get::<_, String>(0)?).unwrap()),
                username: row.get(1)?,
                password_hash: row.get(2)?,
                is_admin: row.get::<_, i32>(3)? != 0,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        },
    );

    match result {
        Ok(user) => Ok(Some(user)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Get a user by username.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `username` - Username to search for
///
/// # Returns
///
/// * `Ok(Some(User))` - The user if found
/// * `Ok(None)` - If the user does not exist
/// * `Err(Error)` - If a database error occurs
pub fn get_user_by_username(conn: &Connection, username: &str) -> Result<Option<User>> {
    let result = conn.query_row(
        "SELECT id, username, password_hash, is_admin, created_at
         FROM users WHERE username = :username",
        rusqlite::named_params! { ":username": username },
        |row| {
            Ok(User {
                id: UserId::from(Uuid::parse_str(&row.get::<_, String>(0)?).unwrap()),
                username: row.get(1)?,
                password_hash: row.get(2)?,
                is_admin: row.get::<_, i32>(3)? != 0,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        },
    );

    match result {
        Ok(user) => Ok(Some(user)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// List all users.
///
/// # Arguments
///
/// * `conn` - Database connection
///
/// # Returns
///
/// * `Ok(Vec<User>)` - List of all users
/// * `Err(Error)` - If a database error occurs
pub fn list_users(conn: &Connection) -> Result<Vec<User>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, username, password_hash, is_admin, created_at FROM users ORDER BY username",
        )
        .map_err(|e| Error::database(e.to_string()))?;

    let users = stmt
        .query_map([], |row| {
            Ok(User {
                id: UserId::from(Uuid::parse_str(&row.get::<_, String>(0)?).unwrap()),
                username: row.get(1)?,
                password_hash: row.get(2)?,
                is_admin: row.get::<_, i32>(3)? != 0,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        })
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(users)
}

/// Delete a user.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `id` - User ID to delete
///
/// # Returns
///
/// * `Ok(true)` - If the user was deleted
/// * `Ok(false)` - If the user did not exist
/// * `Err(Error)` - If a database error occurs
pub fn delete_user(conn: &Connection, id: UserId) -> Result<bool> {
    let rows_affected = conn
        .execute(
            "DELETE FROM users WHERE id = :id",
            rusqlite::named_params! { ":id": id.to_string() },
        )
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(rows_affected > 0)
}

/// Update a user's admin status.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `id` - User ID
/// * `is_admin` - New admin status
///
/// # Returns
///
/// * `Ok(())` - If the update succeeded
/// * `Err(Error)` - If the user does not exist or a database error occurs
pub fn set_user_admin(conn: &Connection, id: UserId, is_admin: bool) -> Result<()> {
    let rows_affected = conn
        .execute(
            "UPDATE users SET is_admin = :is_admin WHERE id = :id",
            rusqlite::named_params! {
                ":id": id.to_string(),
                ":is_admin": is_admin,
            },
        )
        .map_err(|e| Error::database(e.to_string()))?;

    if rows_affected == 0 {
        return Err(Error::not_found("user"));
    }

    Ok(())
}

/// Update a user's password hash.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `id` - User ID
/// * `password_hash` - New password hash
///
/// # Returns
///
/// * `Ok(())` - If the update succeeded
/// * `Err(Error)` - If the user does not exist or a database error occurs
pub fn update_password(conn: &Connection, id: UserId, password_hash: &str) -> Result<()> {
    let rows_affected = conn
        .execute(
            "UPDATE users SET password_hash = :password_hash WHERE id = :id",
            rusqlite::named_params! {
                ":id": id.to_string(),
                ":password_hash": password_hash,
            },
        )
        .map_err(|e| Error::database(e.to_string()))?;

    if rows_affected == 0 {
        return Err(Error::not_found("user"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_memory_pool;

    #[test]
    fn test_create_user() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let user = create_user(&conn, "testuser", "hash123", false).unwrap();
        assert_eq!(user.username, "testuser");
        assert_eq!(user.password_hash, "hash123");
        assert!(!user.is_admin);
    }

    #[test]
    fn test_create_duplicate_username() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        create_user(&conn, "testuser", "hash123", false).unwrap();
        let result = create_user(&conn, "testuser", "hash456", false);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_user() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let created = create_user(&conn, "testuser", "hash123", true).unwrap();
        let found = get_user(&conn, created.id).unwrap();

        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.id, created.id);
        assert_eq!(found.username, "testuser");
        assert!(found.is_admin);
    }

    #[test]
    fn test_get_user_not_found() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let result = get_user(&conn, UserId::new()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_user_by_username() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        create_user(&conn, "testuser", "hash123", false).unwrap();
        let found = get_user_by_username(&conn, "testuser").unwrap();

        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.username, "testuser");
    }

    #[test]
    fn test_get_user_by_username_not_found() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let result = get_user_by_username(&conn, "nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_list_users() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        create_user(&conn, "user1", "hash1", false).unwrap();
        create_user(&conn, "user2", "hash2", true).unwrap();
        create_user(&conn, "user3", "hash3", false).unwrap();

        let users = list_users(&conn).unwrap();
        assert_eq!(users.len(), 3);

        // Should be sorted by username
        assert_eq!(users[0].username, "user1");
        assert_eq!(users[1].username, "user2");
        assert_eq!(users[2].username, "user3");
    }

    #[test]
    fn test_delete_user() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let user = create_user(&conn, "testuser", "hash123", false).unwrap();
        let deleted = delete_user(&conn, user.id).unwrap();
        assert!(deleted);

        let found = get_user(&conn, user.id).unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_delete_user_not_found() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let deleted = delete_user(&conn, UserId::new()).unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_set_user_admin() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let user = create_user(&conn, "testuser", "hash123", false).unwrap();
        assert!(!user.is_admin);

        set_user_admin(&conn, user.id, true).unwrap();

        let updated = get_user(&conn, user.id).unwrap().unwrap();
        assert!(updated.is_admin);
    }

    #[test]
    fn test_set_user_admin_not_found() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let result = set_user_admin(&conn, UserId::new(), true);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_password() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let user = create_user(&conn, "testuser", "hash123", false).unwrap();
        update_password(&conn, user.id, "newhash456").unwrap();

        let updated = get_user(&conn, user.id).unwrap().unwrap();
        assert_eq!(updated.password_hash, "newhash456");
    }

    #[test]
    fn test_update_password_not_found() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let result = update_password(&conn, UserId::new(), "hash");
        assert!(result.is_err());
    }
}
