-- Migration 003: Refactor media_streams to reference media_files instead of items
-- This allows multiple files per item, each with their own streams

-- Create new media_streams table with media_file_id reference
CREATE TABLE media_streams_new (
    id              TEXT PRIMARY KEY NOT NULL,
    media_file_id   TEXT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    stream_type     TEXT NOT NULL CHECK(stream_type IN ('video', 'audio', 'subtitle')),
    index_num       INTEGER NOT NULL,
    codec           TEXT,
    language        TEXT,
    title           TEXT,
    is_default      INTEGER NOT NULL DEFAULT 0,
    is_forced       INTEGER NOT NULL DEFAULT 0,
    is_external     INTEGER NOT NULL DEFAULT 0,
    external_path   TEXT,
    width           INTEGER,
    height          INTEGER,
    bit_rate        INTEGER,
    frame_rate      REAL,
    pixel_format    TEXT,
    color_primaries TEXT,
    color_transfer  TEXT,
    color_space     TEXT,
    channels        INTEGER,
    channel_layout  TEXT,
    sample_rate     INTEGER
);

-- Migrate existing data: create media_files for items with file_path, then link streams
-- For items that have a file_path, create a 'source' media_file entry
INSERT INTO media_files (id, item_id, role, file_path, file_size, container, video_codec, audio_codec, width, height, duration_ticks)
SELECT
    id || '-source',
    id,
    'source',
    file_path,
    COALESCE(size_bytes, 0),
    COALESCE(container, 'unknown'),
    video_codec,
    audio_codec,
    CAST(SUBSTR(resolution, 1, INSTR(resolution || 'x', 'x') - 1) AS INTEGER),
    CAST(SUBSTR(resolution, INSTR(resolution, 'x') + 1) AS INTEGER),
    runtime_ticks
FROM items
WHERE file_path IS NOT NULL;

-- Migrate streams to reference new media_files
INSERT INTO media_streams_new (id, media_file_id, stream_type, index_num, codec, language, title,
    is_default, is_forced, is_external, external_path, width, height, bit_rate, frame_rate,
    pixel_format, color_primaries, color_transfer, color_space, channels, channel_layout, sample_rate)
SELECT
    ms.id,
    mf.id,
    ms.stream_type,
    ms.index_num,
    ms.codec,
    ms.language,
    ms.title,
    ms.is_default,
    ms.is_forced,
    ms.is_external,
    ms.external_path,
    ms.width,
    ms.height,
    ms.bit_rate,
    ms.frame_rate,
    ms.pixel_format,
    ms.color_primaries,
    ms.color_transfer,
    ms.color_space,
    ms.channels,
    ms.channel_layout,
    ms.sample_rate
FROM media_streams ms
JOIN media_files mf ON mf.item_id = ms.item_id AND mf.role = 'source';

-- Drop old table and rename new one
DROP TABLE media_streams;
ALTER TABLE media_streams_new RENAME TO media_streams;

-- Create indexes on new table
CREATE INDEX idx_media_streams_file ON media_streams(media_file_id);
CREATE INDEX idx_media_streams_type ON media_streams(stream_type);
