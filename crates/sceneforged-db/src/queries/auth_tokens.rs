//! Authentication token database queries.
//!
//! This module provides CRUD operations for authentication tokens used in
//! MediaBrowser/Jellyfin authentication.

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use sceneforged_common::{Error, Result, UserId};
use uuid::Uuid;

use crate::models::{AuthToken, User};

/// Create a new authentication token.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `user_id` - ID of the user this token belongs to
/// * `device_id` - Unique identifier for the device
/// * `device_name` - Optional friendly name for the device
/// * `client_name` - Optional name of the client application
/// * `client_version` - Optional version of the client application
///
/// # Returns
///
/// * `Ok(AuthToken)` - The created authentication token
/// * `Err(Error)` - If a database error occurs
pub fn create_token(
    conn: &Connection,
    user_id: UserId,
    device_id: &str,
    device_name: Option<&str>,
    client_name: Option<&str>,
    client_version: Option<&str>,
) -> Result<AuthToken> {
    let token = Uuid::new_v4().to_string();
    let now = Utc::now();

    conn.execute(
        "INSERT INTO auth_tokens (token, user_id, device_id, device_name, client_name, client_version, created_at, last_activity)
         VALUES (:token, :user_id, :device_id, :device_name, :client_name, :client_version, :created_at, :last_activity)",
        rusqlite::named_params! {
            ":token": token,
            ":user_id": user_id.to_string(),
            ":device_id": device_id,
            ":device_name": device_name,
            ":client_name": client_name,
            ":client_version": client_version,
            ":created_at": now.to_rfc3339(),
            ":last_activity": now.to_rfc3339(),
        },
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(AuthToken {
        token,
        user_id,
        device_id: device_id.to_string(),
        device_name: device_name.map(String::from),
        client_name: client_name.map(String::from),
        client_version: client_version.map(String::from),
        created_at: now,
        last_activity: now,
    })
}

/// Get an authentication token and its associated user.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `token` - The token string to look up
///
/// # Returns
///
/// * `Ok(Some((AuthToken, User)))` - The token and user if found
/// * `Ok(None)` - If the token does not exist
/// * `Err(Error)` - If a database error occurs
pub fn get_token_with_user(conn: &Connection, token: &str) -> Result<Option<(AuthToken, User)>> {
    let result = conn.query_row(
        "SELECT
            t.token, t.user_id, t.device_id, t.device_name, t.client_name, t.client_version,
            t.created_at, t.last_activity,
            u.id, u.username, u.password_hash, u.is_admin, u.created_at
         FROM auth_tokens t
         INNER JOIN users u ON t.user_id = u.id
         WHERE t.token = :token",
        rusqlite::named_params! { ":token": token },
        |row| {
            let auth_token = AuthToken {
                token: row.get(0)?,
                user_id: UserId::from(Uuid::parse_str(&row.get::<_, String>(1)?).unwrap()),
                device_id: row.get(2)?,
                device_name: row.get(3)?,
                client_name: row.get(4)?,
                client_version: row.get(5)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                    .unwrap()
                    .with_timezone(&Utc),
                last_activity: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                    .unwrap()
                    .with_timezone(&Utc),
            };

            let user = User {
                id: UserId::from(Uuid::parse_str(&row.get::<_, String>(8)?).unwrap()),
                username: row.get(9)?,
                password_hash: row.get(10)?,
                is_admin: row.get::<_, i32>(11)? != 0,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(12)?)
                    .unwrap()
                    .with_timezone(&Utc),
            };

            Ok((auth_token, user))
        },
    );

    match result {
        Ok(data) => Ok(Some(data)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Delete an authentication token (logout).
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `token` - The token string to delete
///
/// # Returns
///
/// * `Ok(true)` - If the token was deleted
/// * `Ok(false)` - If the token did not exist
/// * `Err(Error)` - If a database error occurs
pub fn delete_token(conn: &Connection, token: &str) -> Result<bool> {
    let rows_affected = conn
        .execute(
            "DELETE FROM auth_tokens WHERE token = :token",
            rusqlite::named_params! { ":token": token },
        )
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(rows_affected > 0)
}

/// Update the last activity timestamp for a token.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `token` - The token string to update
///
/// # Returns
///
/// * `Ok(())` - If the update succeeded
/// * `Err(Error)` - If a database error occurs
pub fn update_token_activity(conn: &Connection, token: &str) -> Result<()> {
    let now = Utc::now();

    conn.execute(
        "UPDATE auth_tokens SET last_activity = :last_activity WHERE token = :token",
        rusqlite::named_params! {
            ":token": token,
            ":last_activity": now.to_rfc3339(),
        },
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(())
}

/// Delete all tokens for a specific user.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `user_id` - The user ID whose tokens should be deleted
///
/// # Returns
///
/// * `Ok(usize)` - Number of tokens deleted
/// * `Err(Error)` - If a database error occurs
pub fn delete_user_tokens(conn: &Connection, user_id: UserId) -> Result<usize> {
    let rows_affected = conn
        .execute(
            "DELETE FROM auth_tokens WHERE user_id = :user_id",
            rusqlite::named_params! { ":user_id": user_id.to_string() },
        )
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(rows_affected)
}

/// Get all tokens for a specific user.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `user_id` - The user ID to get tokens for
///
/// # Returns
///
/// * `Ok(Vec<AuthToken>)` - List of tokens for the user
/// * `Err(Error)` - If a database error occurs
pub fn list_user_tokens(conn: &Connection, user_id: UserId) -> Result<Vec<AuthToken>> {
    let mut stmt = conn
        .prepare(
            "SELECT token, user_id, device_id, device_name, client_name, client_version, created_at, last_activity
             FROM auth_tokens
             WHERE user_id = :user_id
             ORDER BY last_activity DESC",
        )
        .map_err(|e| Error::database(e.to_string()))?;

    let tokens = stmt
        .query_map(
            rusqlite::named_params! { ":user_id": user_id.to_string() },
            |row| {
                Ok(AuthToken {
                    token: row.get(0)?,
                    user_id: UserId::from(Uuid::parse_str(&row.get::<_, String>(1)?).unwrap()),
                    device_id: row.get(2)?,
                    device_name: row.get(3)?,
                    client_name: row.get(4)?,
                    client_version: row.get(5)?,
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                        .unwrap()
                        .with_timezone(&Utc),
                    last_activity: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                        .unwrap()
                        .with_timezone(&Utc),
                })
            },
        )
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_memory_pool;
    use crate::queries::users;

    #[test]
    fn test_create_token() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let user = users::create_user(&conn, "testuser", "hash123", false).unwrap();
        let token = create_token(
            &conn,
            user.id,
            "device123",
            Some("Test Device"),
            Some("Test Client"),
            Some("1.0"),
        )
        .unwrap();

        assert_eq!(token.user_id, user.id);
        assert_eq!(token.device_id, "device123");
        assert_eq!(token.device_name, Some("Test Device".to_string()));
        assert_eq!(token.client_name, Some("Test Client".to_string()));
        assert_eq!(token.client_version, Some("1.0".to_string()));
    }

    #[test]
    fn test_get_token_with_user() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let user = users::create_user(&conn, "testuser", "hash123", true).unwrap();
        let token = create_token(&conn, user.id, "device123", None, None, None).unwrap();

        let result = get_token_with_user(&conn, &token.token).unwrap();
        assert!(result.is_some());

        let (found_token, found_user) = result.unwrap();
        assert_eq!(found_token.token, token.token);
        assert_eq!(found_token.user_id, user.id);
        assert_eq!(found_user.id, user.id);
        assert_eq!(found_user.username, "testuser");
        assert!(found_user.is_admin);
    }

    #[test]
    fn test_get_token_not_found() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let result = get_token_with_user(&conn, "nonexistent-token").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_delete_token() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let user = users::create_user(&conn, "testuser", "hash123", false).unwrap();
        let token = create_token(&conn, user.id, "device123", None, None, None).unwrap();

        let deleted = delete_token(&conn, &token.token).unwrap();
        assert!(deleted);

        let result = get_token_with_user(&conn, &token.token).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_delete_nonexistent_token() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let deleted = delete_token(&conn, "nonexistent-token").unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_update_token_activity() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let user = users::create_user(&conn, "testuser", "hash123", false).unwrap();
        let token = create_token(&conn, user.id, "device123", None, None, None).unwrap();

        let original_activity = token.last_activity;

        // Wait a tiny bit to ensure timestamp changes
        std::thread::sleep(std::time::Duration::from_millis(10));

        update_token_activity(&conn, &token.token).unwrap();

        let result = get_token_with_user(&conn, &token.token).unwrap().unwrap();
        assert!(result.0.last_activity > original_activity);
    }

    #[test]
    fn test_delete_user_tokens() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let user = users::create_user(&conn, "testuser", "hash123", false).unwrap();
        create_token(&conn, user.id, "device1", None, None, None).unwrap();
        create_token(&conn, user.id, "device2", None, None, None).unwrap();
        create_token(&conn, user.id, "device3", None, None, None).unwrap();

        let deleted = delete_user_tokens(&conn, user.id).unwrap();
        assert_eq!(deleted, 3);

        let tokens = list_user_tokens(&conn, user.id).unwrap();
        assert_eq!(tokens.len(), 0);
    }

    #[test]
    fn test_list_user_tokens() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let user1 = users::create_user(&conn, "user1", "hash1", false).unwrap();
        let user2 = users::create_user(&conn, "user2", "hash2", false).unwrap();

        create_token(&conn, user1.id, "device1", None, None, None).unwrap();
        create_token(&conn, user1.id, "device2", None, None, None).unwrap();
        create_token(&conn, user2.id, "device3", None, None, None).unwrap();

        let tokens = list_user_tokens(&conn, user1.id).unwrap();
        assert_eq!(tokens.len(), 2);

        let tokens = list_user_tokens(&conn, user2.id).unwrap();
        assert_eq!(tokens.len(), 1);
    }
}
