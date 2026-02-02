//! Database migrations module
//!
//! This module handles SQLite database schema migrations for sceneforged.
//! Migrations are embedded in the binary and executed in order.

use rusqlite::{Connection, Result};
use thiserror::Error;

/// Migration error types
#[derive(Error, Debug)]
pub enum MigrationError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Migration {0} failed: {1}")]
    Failed(usize, String),
}

/// A single migration with its SQL content
struct Migration {
    version: usize,
    name: &'static str,
    sql: &'static str,
}

/// All available migrations
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial",
        sql: include_str!("001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "media_files",
        sql: include_str!("002_media_files.sql"),
    },
    Migration {
        version: 3,
        name: "streams_refactor",
        sql: include_str!("003_streams_refactor.sql"),
    },
];

/// Initialize the migrations table if it doesn't exist
fn init_migrations_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;
    Ok(())
}

/// Get the current schema version
fn get_current_version(conn: &Connection) -> Result<usize> {
    match conn.query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
        row.get::<_, Option<usize>>(0)
    }) {
        Ok(Some(version)) => Ok(version),
        Ok(None) => Ok(0),
        Err(e) => Err(e),
    }
}

/// Apply a single migration
fn apply_migration(conn: &Connection, migration: &Migration) -> Result<(), MigrationError> {
    // Execute the migration SQL
    conn.execute_batch(migration.sql)
        .map_err(|e| MigrationError::Failed(migration.version, e.to_string()))?;

    // Record that this migration was applied
    conn.execute(
        "INSERT INTO schema_migrations (version, name) VALUES (?, ?)",
        rusqlite::params![migration.version, migration.name],
    )
    .map_err(|e| MigrationError::Failed(migration.version, e.to_string()))?;

    Ok(())
}

/// Run all pending migrations
///
/// This function will:
/// 1. Create the migrations table if it doesn't exist
/// 2. Determine which migrations need to be applied
/// 3. Apply each migration in order within a transaction
/// 4. Enable foreign key constraints
///
/// # Arguments
///
/// * `conn` - Database connection to run migrations on
///
/// # Returns
///
/// * `Ok(usize)` - Number of migrations applied
/// * `Err(MigrationError)` - If any migration fails
pub fn run_migrations(conn: &Connection) -> Result<usize, MigrationError> {
    // Enable foreign key constraints
    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(MigrationError::Database)?;

    // Initialize migrations tracking table
    init_migrations_table(conn).map_err(MigrationError::Database)?;

    // Get current schema version
    let current_version = get_current_version(conn).map_err(MigrationError::Database)?;

    // Find migrations that need to be applied
    let pending_migrations: Vec<_> = MIGRATIONS
        .iter()
        .filter(|m| m.version > current_version)
        .collect();

    if pending_migrations.is_empty() {
        return Ok(0);
    }

    // Apply each pending migration in a transaction
    let mut applied_count = 0;
    for migration in pending_migrations {
        let tx = conn
            .unchecked_transaction()
            .map_err(MigrationError::Database)?;

        apply_migration(&tx, migration)?;

        tx.commit()
            .map_err(|e| MigrationError::Failed(migration.version, e.to_string()))?;

        applied_count += 1;

        eprintln!(
            "Applied migration {}: {}",
            migration.version, migration.name
        );
    }

    Ok(applied_count)
}

/// Get the current schema version without applying migrations
pub fn current_version(conn: &Connection) -> Result<usize, MigrationError> {
    init_migrations_table(conn).map_err(MigrationError::Database)?;

    get_current_version(conn).map_err(MigrationError::Database)
}

/// Get the latest available migration version
pub fn latest_version() -> usize {
    MIGRATIONS.last().map(|m| m.version).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_run_migrations() {
        let conn = Connection::open_in_memory().unwrap();

        // First run should apply all migrations
        let applied = run_migrations(&conn).unwrap();
        assert_eq!(applied, MIGRATIONS.len());

        // Verify current version
        let version = current_version(&conn).unwrap();
        assert_eq!(version, latest_version());

        // Second run should not apply any migrations
        let applied = run_migrations(&conn).unwrap();
        assert_eq!(applied, 0);
    }

    #[test]
    fn test_schema_created() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        // Verify all tables exist
        let tables = vec![
            "users",
            "libraries",
            "items",
            "media_streams",
            "images",
            "user_item_data",
            "auth_tokens",
            "sync_change_log",
            "sync_user_data_log",
            "sync_checkpoints",
            "schema_migrations",
        ];

        for table in tables {
            let count: i64 = conn
                .query_row(
                    &format!(
                        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='{}'",
                        table
                    ),
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "Table {} should exist", table);
        }
    }

    #[test]
    fn test_foreign_keys_enabled() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        // Verify foreign keys are enabled
        let enabled: i32 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(enabled, 1);
    }
}
