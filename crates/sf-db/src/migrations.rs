//! Embedded SQL migrations and runner.
//!
//! Migrations are stored as `&str` constants and executed in order.  A
//! `schema_migrations` table tracks which versions have been applied.

use rusqlite::Connection;
use sf_core::{Error, Result};

/// V1: initial schema -- creates all core tables and indexes.
const V1_INITIAL: &str = r#"
-- Users and auth
CREATE TABLE users (
    id            TEXT PRIMARY KEY,
    username      TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    role          TEXT NOT NULL DEFAULT 'user',
    created_at    TEXT NOT NULL
);

CREATE TABLE auth_tokens (
    id         TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id),
    token      TEXT UNIQUE NOT NULL,
    expires_at TEXT NOT NULL
);

-- Libraries
CREATE TABLE libraries (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    media_type TEXT NOT NULL,
    paths      TEXT NOT NULL,
    config     TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL
);

-- Items (movies, series, seasons, episodes)
CREATE TABLE items (
    id               TEXT PRIMARY KEY,
    library_id       TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    item_kind        TEXT NOT NULL,
    name             TEXT NOT NULL,
    sort_name        TEXT,
    year             INTEGER,
    overview         TEXT,
    runtime_minutes  INTEGER,
    community_rating REAL,
    provider_ids     TEXT DEFAULT '{}',
    parent_id        TEXT REFERENCES items(id),
    season_number    INTEGER,
    episode_number   INTEGER,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);

-- Media files
CREATE TABLE media_files (
    id                TEXT PRIMARY KEY,
    item_id           TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    file_path         TEXT NOT NULL UNIQUE,
    file_name         TEXT NOT NULL,
    file_size         INTEGER NOT NULL,
    container         TEXT,
    video_codec       TEXT,
    audio_codec       TEXT,
    resolution_width  INTEGER,
    resolution_height INTEGER,
    hdr_format        TEXT,
    has_dolby_vision  INTEGER DEFAULT 0,
    dv_profile        INTEGER,
    role              TEXT NOT NULL DEFAULT 'source',
    profile           TEXT NOT NULL DEFAULT 'C',
    duration_secs     REAL,
    created_at        TEXT NOT NULL
);

-- Images / artwork
CREATE TABLE images (
    id         TEXT PRIMARY KEY,
    item_id    TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    image_type TEXT NOT NULL,
    path       TEXT NOT NULL,
    provider   TEXT,
    width      INTEGER,
    height     INTEGER
);

-- File-processing jobs (scan / ingest pipeline)
CREATE TABLE jobs (
    id            TEXT PRIMARY KEY,
    file_path     TEXT NOT NULL,
    file_name     TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'queued',
    rule_name     TEXT,
    progress      REAL DEFAULT 0.0,
    current_step  TEXT,
    error         TEXT,
    source        TEXT,
    retry_count   INTEGER DEFAULT 0,
    max_retries   INTEGER DEFAULT 3,
    priority      INTEGER DEFAULT 0,
    locked_by     TEXT,
    locked_at     TEXT,
    created_at    TEXT NOT NULL,
    started_at    TEXT,
    completed_at  TEXT,
    scheduled_for TEXT
);

-- Conversion jobs (encode pipeline)
CREATE TABLE conversion_jobs (
    id            TEXT PRIMARY KEY,
    item_id       TEXT NOT NULL REFERENCES items(id),
    media_file_id TEXT REFERENCES media_files(id),
    status        TEXT NOT NULL DEFAULT 'queued',
    progress_pct  REAL DEFAULT 0.0,
    encode_fps    REAL,
    eta_secs      INTEGER,
    error         TEXT,
    created_at    TEXT NOT NULL,
    started_at    TEXT,
    completed_at  TEXT
);

-- HLS cache
CREATE TABLE hls_cache (
    media_file_id TEXT PRIMARY KEY REFERENCES media_files(id),
    playlist      TEXT NOT NULL,
    segments      TEXT NOT NULL,
    created_at    TEXT NOT NULL
);

-- Playback state
CREATE TABLE playback (
    user_id        TEXT NOT NULL REFERENCES users(id),
    item_id        TEXT NOT NULL REFERENCES items(id),
    position_secs  REAL NOT NULL DEFAULT 0.0,
    completed      INTEGER DEFAULT 0,
    play_count     INTEGER DEFAULT 0,
    last_played_at TEXT NOT NULL,
    PRIMARY KEY (user_id, item_id)
);

-- Indexes
CREATE INDEX idx_items_library_id ON items(library_id);
CREATE INDEX idx_items_parent_id  ON items(parent_id);
CREATE INDEX idx_media_files_item ON media_files(item_id);
CREATE INDEX idx_images_item      ON images(item_id);
CREATE INDEX idx_jobs_status      ON jobs(status);
CREATE INDEX idx_jobs_file_path   ON jobs(file_path);
"#;

/// V2: enhance conversion_jobs with locking and source tracking; add indexes.
const V2_CONVERSION_JOBS: &str = r#"
ALTER TABLE conversion_jobs ADD COLUMN locked_by TEXT;
ALTER TABLE conversion_jobs ADD COLUMN locked_at TEXT;
ALTER TABLE conversion_jobs ADD COLUMN source_media_file_id TEXT REFERENCES media_files(id);
CREATE INDEX idx_conversion_jobs_status ON conversion_jobs(status);
CREATE INDEX idx_conversion_jobs_item ON conversion_jobs(item_id);
"#;

/// V3: add favorites table and playback indexes.
const V3_FAVORITES: &str = r#"
CREATE TABLE favorites (
    user_id TEXT NOT NULL REFERENCES users(id),
    item_id TEXT NOT NULL REFERENCES items(id),
    created_at TEXT NOT NULL,
    PRIMARY KEY (user_id, item_id)
);
CREATE INDEX idx_playback_user ON playback(user_id);
CREATE INDEX idx_favorites_user ON favorites(user_id);
"#;

/// V4: seed the anonymous user used when auth is disabled.
///
/// The auth middleware returns this well-known UUID for unauthenticated
/// requests.  Without a corresponding row in `users`, any INSERT into
/// `playback` or `favorites` violates the FK constraint.
const V4_ANONYMOUS_USER: &str = r#"
INSERT OR IGNORE INTO users (id, username, password_hash, role, created_at)
VALUES ('00000000-0000-0000-0000-000000000000', 'anonymous', '!disabled', 'user', datetime('now'));
"#;

/// V5: subtitle tracks table.
const V5_SUBTITLE_TRACKS: &str = r#"
CREATE TABLE subtitle_tracks (
    id              TEXT PRIMARY KEY,
    media_file_id   TEXT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    track_index     INTEGER NOT NULL,
    codec           TEXT NOT NULL,
    language        TEXT,
    forced          INTEGER DEFAULT 0,
    default_track   INTEGER DEFAULT 0,
    created_at      TEXT NOT NULL
);
CREATE INDEX idx_subtitle_tracks_media ON subtitle_tracks(media_file_id);
"#;

/// V6: Full-text search index on items (name + overview).
const V6_ITEMS_FTS: &str = r#"
CREATE VIRTUAL TABLE IF NOT EXISTS items_fts USING fts5(
    name,
    overview,
    content='items',
    content_rowid='rowid'
);

-- Populate from existing data.
INSERT INTO items_fts(rowid, name, overview)
    SELECT rowid, name, COALESCE(overview, '') FROM items;

-- Keep FTS in sync with items table.
CREATE TRIGGER items_fts_insert AFTER INSERT ON items BEGIN
    INSERT INTO items_fts(rowid, name, overview)
        VALUES (NEW.rowid, NEW.name, COALESCE(NEW.overview, ''));
END;
CREATE TRIGGER items_fts_delete AFTER DELETE ON items BEGIN
    INSERT INTO items_fts(items_fts, rowid, name, overview)
        VALUES ('delete', OLD.rowid, OLD.name, COALESCE(OLD.overview, ''));
END;
CREATE TRIGGER items_fts_update AFTER UPDATE ON items BEGIN
    INSERT INTO items_fts(items_fts, rowid, name, overview)
        VALUES ('delete', OLD.rowid, OLD.name, COALESCE(OLD.overview, ''));
    INSERT INTO items_fts(rowid, name, overview)
        VALUES (NEW.rowid, NEW.name, COALESCE(NEW.overview, ''));
END;
"#;

/// V7: Composite indexes for scanner performance.
const V7_SCANNER_INDEXES: &str = r#"
CREATE INDEX IF NOT EXISTS idx_items_library_name_kind ON items(library_id, name, item_kind);
CREATE INDEX IF NOT EXISTS idx_items_parent_kind_season ON items(parent_id, item_kind, season_number);
"#;

/// V8: Add ON DELETE CASCADE to child tables that were missing it.
///
/// SQLite cannot ALTER constraints, so each table is recreated.
const V8_FK_CASCADES: &str = r#"
-- 1. hls_cache: add CASCADE on media_file_id
CREATE TABLE hls_cache_new (
    media_file_id TEXT PRIMARY KEY REFERENCES media_files(id) ON DELETE CASCADE,
    playlist      TEXT NOT NULL,
    segments      TEXT NOT NULL,
    created_at    TEXT NOT NULL
);
INSERT INTO hls_cache_new SELECT * FROM hls_cache;
DROP TABLE hls_cache;
ALTER TABLE hls_cache_new RENAME TO hls_cache;

-- 2. conversion_jobs: add CASCADE on item_id, media_file_id, source_media_file_id
CREATE TABLE conversion_jobs_new (
    id                   TEXT PRIMARY KEY,
    item_id              TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    media_file_id        TEXT REFERENCES media_files(id) ON DELETE CASCADE,
    status               TEXT NOT NULL DEFAULT 'queued',
    progress_pct         REAL DEFAULT 0.0,
    encode_fps           REAL,
    eta_secs             INTEGER,
    error                TEXT,
    created_at           TEXT NOT NULL,
    started_at           TEXT,
    completed_at         TEXT,
    locked_by            TEXT,
    locked_at            TEXT,
    source_media_file_id TEXT REFERENCES media_files(id) ON DELETE CASCADE
);
INSERT INTO conversion_jobs_new SELECT * FROM conversion_jobs;
DROP TABLE conversion_jobs;
ALTER TABLE conversion_jobs_new RENAME TO conversion_jobs;
CREATE INDEX idx_conversion_jobs_status ON conversion_jobs(status);
CREATE INDEX idx_conversion_jobs_item ON conversion_jobs(item_id);

-- 3. playback: add CASCADE on item_id
CREATE TABLE playback_new (
    user_id        TEXT NOT NULL REFERENCES users(id),
    item_id        TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    position_secs  REAL NOT NULL DEFAULT 0.0,
    completed      INTEGER DEFAULT 0,
    play_count     INTEGER DEFAULT 0,
    last_played_at TEXT NOT NULL,
    PRIMARY KEY (user_id, item_id)
);
INSERT INTO playback_new SELECT * FROM playback;
DROP TABLE playback;
ALTER TABLE playback_new RENAME TO playback;
CREATE INDEX idx_playback_user ON playback(user_id);

-- 4. favorites: add CASCADE on item_id
CREATE TABLE favorites_new (
    user_id    TEXT NOT NULL REFERENCES users(id),
    item_id    TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    PRIMARY KEY (user_id, item_id)
);
INSERT INTO favorites_new SELECT * FROM favorites;
DROP TABLE favorites;
ALTER TABLE favorites_new RENAME TO favorites;
CREATE INDEX idx_favorites_user ON favorites(user_id);
"#;

/// V9: Invitation system + conversion job priority.
const V9_INVITATIONS_PRIORITY: &str = r#"
CREATE TABLE invitations (
    id         TEXT PRIMARY KEY,
    code       TEXT UNIQUE NOT NULL,
    role       TEXT NOT NULL DEFAULT 'user',
    created_by TEXT NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    used_at    TEXT,
    used_by    TEXT REFERENCES users(id)
);

ALTER TABLE conversion_jobs ADD COLUMN priority INTEGER NOT NULL DEFAULT 0;
"#;

/// V10: Add encoding stats columns to conversion_jobs.
const V10_CONVERSION_STATS: &str = r#"
ALTER TABLE conversion_jobs ADD COLUMN bitrate TEXT;
ALTER TABLE conversion_jobs ADD COLUMN speed TEXT;
ALTER TABLE conversion_jobs ADD COLUMN output_size INTEGER;
"#;

/// V11: Add scan status columns to items for progressive scanning.
const V11_SCAN_STATUS: &str = r#"
ALTER TABLE items ADD COLUMN scan_status TEXT;
ALTER TABLE items ADD COLUMN scan_error TEXT;
ALTER TABLE items ADD COLUMN source_file_path TEXT;
"#;

/// Ordered list of (version, sql) pairs.
const MIGRATIONS: &[(i64, &str)] = &[
    (1, V1_INITIAL),
    (2, V2_CONVERSION_JOBS),
    (3, V3_FAVORITES),
    (4, V4_ANONYMOUS_USER),
    (5, V5_SUBTITLE_TRACKS),
    (6, V6_ITEMS_FTS),
    (7, V7_SCANNER_INDEXES),
    (8, V8_FK_CASCADES),
    (9, V9_INVITATIONS_PRIORITY),
    (10, V10_CONVERSION_STATS),
    (11, V11_SCAN_STATUS),
];

/// Run all pending migrations on `conn`.
///
/// Creates the `schema_migrations` tracking table if it does not exist,
/// then applies each outstanding migration inside a transaction.
pub fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .map_err(|e| Error::database(format!("Failed to create schema_migrations: {e}")))?;

    for &(version, sql) in MIGRATIONS {
        let already: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM schema_migrations WHERE version = ?1",
                [version],
                |row| row.get(0),
            )
            .map_err(|e| Error::database(e.to_string()))?;

        if already {
            continue;
        }

        let tx = conn
            .unchecked_transaction()
            .map_err(|e| Error::database(e.to_string()))?;

        tx.execute_batch(sql)
            .map_err(|e| Error::database(format!("Migration V{version} failed: {e}")))?;

        tx.execute(
            "INSERT INTO schema_migrations (version) VALUES (?1)",
            [version],
        )
        .map_err(|e| Error::database(e.to_string()))?;

        tx.commit()
            .map_err(|e| Error::database(e.to_string()))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        // second call is a no-op
        run_migrations(&conn).unwrap();
    }

    #[test]
    fn test_all_tables_created() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        run_migrations(&conn).unwrap();

        let tables = [
            "users",
            "auth_tokens",
            "libraries",
            "items",
            "media_files",
            "images",
            "jobs",
            "conversion_jobs",
            "hls_cache",
            "playback",
            "favorites",
            "invitations",
            "schema_migrations",
        ];
        for t in &tables {
            let exists: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name=?1",
                    [t],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(exists, "table {t} should exist");
        }
    }

    #[test]
    fn test_fk_cascade_deletes() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        run_migrations(&conn).unwrap();

        // Create a library + item + media_file, then child rows in cascade tables.
        conn.execute_batch(
            "INSERT INTO libraries (id, name, media_type, paths, created_at)
             VALUES ('lib1', 'Test', 'movies', '[]', '2025-01-01');
             INSERT INTO users (id, username, password_hash, role, created_at)
             VALUES ('u1', 'test', 'h', 'user', '2025-01-01');
             INSERT INTO items (id, library_id, item_kind, name, created_at, updated_at)
             VALUES ('i1', 'lib1', 'movie', 'Test Movie', '2025-01-01', '2025-01-01');
             INSERT INTO media_files (id, item_id, file_path, file_name, file_size, created_at)
             VALUES ('mf1', 'i1', '/test.mkv', 'test.mkv', 1000, '2025-01-01');
             INSERT INTO hls_cache (media_file_id, playlist, segments, created_at)
             VALUES ('mf1', '#EXTM3U', '[]', '2025-01-01');
             INSERT INTO conversion_jobs (id, item_id, media_file_id, created_at)
             VALUES ('cj1', 'i1', 'mf1', '2025-01-01');
             INSERT INTO playback (user_id, item_id, position_secs, last_played_at)
             VALUES ('u1', 'i1', 120.0, '2025-01-01');
             INSERT INTO favorites (user_id, item_id, created_at)
             VALUES ('u1', 'i1', '2025-01-01');",
        )
        .unwrap();

        // Deleting the library should cascade through items -> media_files -> hls_cache etc.
        conn.execute("DELETE FROM libraries WHERE id = 'lib1'", [])
            .expect("library delete should cascade");

        // Verify all child rows are gone.
        let count = |table: &str| -> i64 {
            conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap()
        };
        assert_eq!(count("items"), 0, "items should be cascade-deleted");
        assert_eq!(count("media_files"), 0, "media_files should be cascade-deleted");
        assert_eq!(count("hls_cache"), 0, "hls_cache should be cascade-deleted");
        assert_eq!(count("conversion_jobs"), 0, "conversion_jobs should be cascade-deleted");
        assert_eq!(count("playback"), 0, "playback should be cascade-deleted");
        assert_eq!(count("favorites"), 0, "favorites should be cascade-deleted");
    }
}
