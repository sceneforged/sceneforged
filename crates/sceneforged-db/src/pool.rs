//! Database connection pool management.
//!
//! This module provides connection pooling for SQLite using r2d2.
//! It handles pool initialization, connection customization, and running migrations.

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use sceneforged_common::{Error, Result};

use crate::migrations;

/// Type alias for the database connection pool.
pub type DbPool = Pool<SqliteConnectionManager>;

/// Type alias for a pooled database connection.
pub type PooledConnection = r2d2::PooledConnection<SqliteConnectionManager>;

/// Initialize a new database pool with the given file path.
///
/// This function will:
/// - Create the SQLite database file if it doesn't exist
/// - Set up connection pooling with r2d2
/// - Enable foreign key constraints on all connections
/// - Run pending database migrations
/// - Set appropriate pool size (default 4 connections)
///
/// # Arguments
///
/// * `db_path` - Path to the SQLite database file
///
/// # Returns
///
/// * `Ok(DbPool)` - Initialized connection pool
/// * `Err(Error)` - If pool creation or migration fails
///
/// # Example
///
/// ```no_run
/// use sceneforged_db::pool::init_pool;
///
/// let pool = init_pool("/var/lib/sceneforged/db.sqlite").unwrap();
/// let conn = pool.get().unwrap();
/// ```
pub fn init_pool(db_path: &str) -> Result<DbPool> {
    let manager = SqliteConnectionManager::file(db_path).with_init(|conn| {
        // Enable foreign key constraints on each new connection
        conn.execute_batch("PRAGMA foreign_keys = ON;")
    });

    let pool = Pool::builder()
        .max_size(4)
        .build(manager)
        .map_err(|e| Error::database(format!("Failed to create connection pool: {}", e)))?;

    // Run migrations on a connection from the pool
    let conn = pool
        .get()
        .map_err(|e| Error::database(format!("Failed to get connection for migrations: {}", e)))?;

    migrations::run_migrations(&conn)
        .map_err(|e| Error::database(format!("Failed to run migrations: {}", e)))?;

    Ok(pool)
}

/// Initialize an in-memory database pool for testing.
///
/// This creates a connection pool backed by an in-memory SQLite database.
/// The database will be lost when the pool is dropped.
///
/// # Returns
///
/// * `Ok(DbPool)` - Initialized in-memory connection pool
/// * `Err(Error)` - If pool creation or migration fails
///
/// # Example
///
/// ```
/// use sceneforged_db::pool::init_memory_pool;
///
/// let pool = init_memory_pool().unwrap();
/// let conn = pool.get().unwrap();
/// ```
pub fn init_memory_pool() -> Result<DbPool> {
    let manager = SqliteConnectionManager::memory().with_init(|conn| {
        // Enable foreign key constraints on each new connection
        conn.execute_batch("PRAGMA foreign_keys = ON;")
    });

    let pool = Pool::builder()
        .max_size(4)
        .build(manager)
        .map_err(|e| Error::database(format!("Failed to create in-memory pool: {}", e)))?;

    // Run migrations on a connection from the pool
    let conn = pool
        .get()
        .map_err(|e| Error::database(format!("Failed to get connection for migrations: {}", e)))?;

    migrations::run_migrations(&conn)
        .map_err(|e| Error::database(format!("Failed to run migrations: {}", e)))?;

    Ok(pool)
}

/// Get a connection from the pool.
///
/// This is a convenience wrapper around `pool.get()` that converts the
/// r2d2 error into our common Error type.
///
/// # Arguments
///
/// * `pool` - The connection pool to get a connection from
///
/// # Returns
///
/// * `Ok(PooledConnection)` - A pooled database connection
/// * `Err(Error)` - If unable to get a connection from the pool
///
/// # Example
///
/// ```
/// use sceneforged_db::pool::{init_memory_pool, get_conn};
///
/// let pool = init_memory_pool().unwrap();
/// let conn = get_conn(&pool).unwrap();
/// ```
pub fn get_conn(pool: &DbPool) -> Result<PooledConnection> {
    pool.get()
        .map_err(|e| Error::database(format!("Failed to get connection from pool: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_memory_pool() {
        let pool = init_memory_pool().unwrap();
        assert_eq!(pool.max_size(), 4);
    }

    #[test]
    fn test_get_conn() {
        let pool = init_memory_pool().unwrap();
        let conn = get_conn(&pool).unwrap();

        // Verify foreign keys are enabled
        let enabled: i32 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(enabled, 1);
    }

    #[test]
    fn test_multiple_connections() {
        let pool = init_memory_pool().unwrap();

        // Get multiple connections
        let _conn1 = get_conn(&pool).unwrap();
        let _conn2 = get_conn(&pool).unwrap();
        let _conn3 = get_conn(&pool).unwrap();

        // Pool should be able to provide multiple connections
        assert!(get_conn(&pool).is_ok());
    }

    #[test]
    fn test_migrations_run_on_init() {
        let pool = init_memory_pool().unwrap();
        let conn = get_conn(&pool).unwrap();

        // Verify that tables exist (migrations were run)
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='users'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_pool_reuses_connections() {
        let pool = init_memory_pool().unwrap();

        {
            let conn = get_conn(&pool).unwrap();
            // Insert test data
            conn.execute(
                "INSERT INTO users (id, username, password_hash) VALUES (?, ?, ?)",
                rusqlite::params!["test-id", "testuser", "hash"],
            )
            .unwrap();
        }

        // Get a new connection and verify data is still there
        let conn = get_conn(&pool).unwrap();
        let username: String = conn
            .query_row(
                "SELECT username FROM users WHERE id = ?",
                ["test-id"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(username, "testuser");
    }
}
