//! Connection pool management for SQLite via r2d2.

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use sf_core::{Error, Result};

use crate::migrations;

/// Type alias for the database connection pool.
pub type DbPool = Pool<SqliteConnectionManager>;

/// Type alias for a pooled database connection.
pub type PooledConnection = r2d2::PooledConnection<SqliteConnectionManager>;

/// Initialize a database pool backed by a file on disk.
///
/// Creates the SQLite file if it does not exist, enables foreign keys and
/// WAL journal mode on every new connection, and runs pending migrations.
pub fn init_pool(db_path: &str) -> Result<DbPool> {
    let manager = SqliteConnectionManager::file(db_path).with_init(|conn| {
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;",
        )
    });

    let pool = Pool::builder()
        .max_size(4)
        .build(manager)
        .map_err(|e| Error::database(format!("Failed to create connection pool: {e}")))?;

    let conn = pool
        .get()
        .map_err(|e| Error::database(format!("Failed to get connection for migrations: {e}")))?;

    migrations::run_migrations(&conn)?;

    Ok(pool)
}

/// Initialize an in-memory database pool (useful for tests).
///
/// Each call creates a uniquely-named shared-cache in-memory database so
/// that parallel tests do not interfere with each other, while all
/// connections *within* a single pool still share state.
pub fn init_memory_pool() -> Result<DbPool> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let uri = format!("file:memdb_{n}?mode=memory&cache=shared");

    let manager = SqliteConnectionManager::file(uri)
        .with_init(|conn| conn.execute_batch("PRAGMA foreign_keys = ON;"));

    let pool = Pool::builder()
        .max_size(4)
        .build(manager)
        .map_err(|e| Error::database(format!("Failed to create in-memory pool: {e}")))?;

    let conn = pool
        .get()
        .map_err(|e| Error::database(format!("Failed to get connection for migrations: {e}")))?;

    migrations::run_migrations(&conn)?;

    Ok(pool)
}

/// Convenience helper to get a connection from the pool.
pub fn get_conn(pool: &DbPool) -> Result<PooledConnection> {
    pool.get()
        .map_err(|e| Error::database(format!("Failed to get connection from pool: {e}")))
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

        let fk: i32 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(fk, 1);
    }

    #[test]
    fn test_migrations_run_on_init() {
        let pool = init_memory_pool().unwrap();
        let conn = get_conn(&pool).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='users'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
