-- Migration 004: Add profile system to media_files
-- Add profile column (A/B/C) and capability flags for profile conversion

-- Create new media_files table with profile columns
CREATE TABLE media_files_new (
    id              TEXT PRIMARY KEY NOT NULL,
    item_id         TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    role            TEXT NOT NULL CHECK(role IN ('source', 'universal', 'extra')),
    profile         TEXT NOT NULL DEFAULT 'C' CHECK(profile IN ('A', 'B', 'C')),
    can_be_profile_a INTEGER NOT NULL DEFAULT 0,
    can_be_profile_b INTEGER NOT NULL DEFAULT 0,
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
    UNIQUE(item_id, profile)
);

-- Migrate existing data with default profile 'C'
INSERT INTO media_files_new (
    id, item_id, role, profile, can_be_profile_a, can_be_profile_b,
    file_path, file_size, container, video_codec, audio_codec,
    width, height, duration_ticks, bit_rate, is_hdr,
    serves_as_universal, has_faststart, keyframe_interval_secs, created_at
)
SELECT
    id, item_id, role, 'C', 0, 0,
    file_path, file_size, container, video_codec, audio_codec,
    width, height, duration_ticks, bit_rate, is_hdr,
    serves_as_universal, has_faststart, keyframe_interval_secs, created_at
FROM media_files;

-- Drop old table and rename new one
DROP TABLE media_files;
ALTER TABLE media_files_new RENAME TO media_files;

-- Create indexes on new table
CREATE INDEX idx_media_files_item ON media_files(item_id);
CREATE INDEX idx_media_files_role ON media_files(role);
CREATE INDEX idx_media_files_profile ON media_files(profile);
