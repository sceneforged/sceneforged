-- Migration 005: Add HLS cache table for precomputed streaming data
-- Stores init segments and segment maps computed at scan/conversion time
-- so that serving never needs to parse the source MP4.

CREATE TABLE hls_cache (
    media_file_id  TEXT PRIMARY KEY NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    init_segment   BLOB NOT NULL,
    segment_count  INTEGER NOT NULL,
    segment_map    BLOB NOT NULL,
    created_at     TEXT NOT NULL DEFAULT (datetime('now'))
);
