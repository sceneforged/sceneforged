-- Migration 002: Add media_files and conversion_jobs tables
-- Multi-file support per item (source, universal, extras)

-- Media files table: tracks individual files for each library item
CREATE TABLE media_files (
    id              TEXT PRIMARY KEY NOT NULL,
    item_id         TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    role            TEXT NOT NULL CHECK(role IN ('source', 'universal', 'extra')),
    file_path       TEXT NOT NULL,
    file_size       INTEGER NOT NULL,
    container       TEXT NOT NULL,
    video_codec     TEXT,
    audio_codec     TEXT,
    width           INTEGER,
    height          INTEGER,
    duration_ticks  INTEGER,
    bit_rate        INTEGER,
    is_hdr          INTEGER NOT NULL DEFAULT 0,
    serves_as_universal INTEGER NOT NULL DEFAULT 0,
    has_faststart   INTEGER NOT NULL DEFAULT 0,
    keyframe_interval_secs REAL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(item_id, role)
);

CREATE INDEX idx_media_files_item ON media_files(item_id);
CREATE INDEX idx_media_files_role ON media_files(role);

-- Conversion jobs table: tracks encoding jobs
CREATE TABLE conversion_jobs (
    id              TEXT PRIMARY KEY NOT NULL,
    item_id         TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    source_file_id  TEXT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    status          TEXT NOT NULL DEFAULT 'queued'
                    CHECK(status IN ('queued', 'running', 'completed', 'failed', 'cancelled')),
    progress_pct    REAL NOT NULL DEFAULT 0.0,
    output_path     TEXT,
    error_message   TEXT,
    hw_accel_used   TEXT,
    encode_fps      REAL,
    started_at      TEXT,
    completed_at    TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_conversion_jobs_item ON conversion_jobs(item_id);
CREATE INDEX idx_conversion_jobs_status ON conversion_jobs(status);
CREATE INDEX idx_conversion_jobs_created ON conversion_jobs(created_at);

-- Add conversion-related columns to items table
ALTER TABLE items ADD COLUMN needs_conversion INTEGER NOT NULL DEFAULT 0;
ALTER TABLE items ADD COLUMN conversion_reason TEXT;
