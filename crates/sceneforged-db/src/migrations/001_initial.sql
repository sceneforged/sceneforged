-- Enable foreign key constraints
PRAGMA foreign_keys = ON;

-- Users table
CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    is_admin INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Libraries table
CREATE TABLE libraries (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    media_type TEXT NOT NULL CHECK(media_type IN ('movies', 'tvshows', 'music')),
    paths TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Items table (movies, series, seasons, episodes)
CREATE TABLE items (
    id TEXT PRIMARY KEY NOT NULL,
    library_id TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    parent_id TEXT REFERENCES items(id) ON DELETE CASCADE,
    item_kind TEXT NOT NULL CHECK(item_kind IN ('movie', 'series', 'season', 'episode', 'music_album', 'music_track')),
    name TEXT NOT NULL,
    sort_name TEXT,
    original_title TEXT,
    file_path TEXT,
    container TEXT,
    video_codec TEXT,
    audio_codec TEXT,
    resolution TEXT,
    runtime_ticks INTEGER,
    size_bytes INTEGER,
    overview TEXT,
    tagline TEXT,
    genres TEXT,
    tags TEXT,
    studios TEXT,
    people TEXT,
    community_rating REAL,
    critic_rating REAL,
    production_year INTEGER,
    premiere_date TEXT,
    end_date TEXT,
    official_rating TEXT,
    provider_ids TEXT,
    scene_release_name TEXT,
    scene_group TEXT,
    index_number INTEGER,
    parent_index_number INTEGER,
    etag TEXT,
    date_created TEXT NOT NULL DEFAULT (datetime('now')),
    date_modified TEXT NOT NULL DEFAULT (datetime('now')),
    hdr_type TEXT CHECK(hdr_type IN ('hdr10', 'hdr10plus', 'dolbyvision', 'hlg')),
    dolby_vision_profile TEXT
);

-- Media streams table
CREATE TABLE media_streams (
    id TEXT PRIMARY KEY NOT NULL,
    item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    stream_type TEXT NOT NULL CHECK(stream_type IN ('video', 'audio', 'subtitle')),
    index_num INTEGER NOT NULL,
    codec TEXT,
    language TEXT,
    title TEXT,
    is_default INTEGER NOT NULL DEFAULT 0,
    is_forced INTEGER NOT NULL DEFAULT 0,
    is_external INTEGER NOT NULL DEFAULT 0,
    external_path TEXT,
    width INTEGER,
    height INTEGER,
    bit_rate INTEGER,
    frame_rate REAL,
    pixel_format TEXT,
    color_primaries TEXT,
    color_transfer TEXT,
    color_space TEXT,
    channels INTEGER,
    channel_layout TEXT,
    sample_rate INTEGER
);

-- Images table
CREATE TABLE images (
    id TEXT PRIMARY KEY NOT NULL,
    item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    image_type TEXT NOT NULL CHECK(image_type IN ('primary', 'backdrop', 'thumb', 'logo', 'art', 'banner', 'disc')),
    path TEXT NOT NULL,
    provider TEXT,
    width INTEGER,
    height INTEGER,
    tag TEXT
);

-- User item data (playback position, favorites, etc.)
CREATE TABLE user_item_data (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    playback_position_ticks INTEGER NOT NULL DEFAULT 0,
    play_count INTEGER NOT NULL DEFAULT 0,
    played INTEGER NOT NULL DEFAULT 0,
    is_favorite INTEGER NOT NULL DEFAULT 0,
    last_played_date TEXT,
    PRIMARY KEY (user_id, item_id)
);

-- Authentication tokens
CREATE TABLE auth_tokens (
    token TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    device_name TEXT,
    client_name TEXT,
    client_version TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_activity TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Sync change log for InfuseSync delta sync
CREATE TABLE sync_change_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id TEXT NOT NULL,
    change_type TEXT NOT NULL CHECK(change_type IN ('added', 'updated', 'removed')),
    changed_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Sync user data log for InfuseSync delta sync
CREATE TABLE sync_user_data_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT NOT NULL,
    item_id TEXT NOT NULL,
    changed_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Sync checkpoints per device
CREATE TABLE sync_checkpoints (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    item_checkpoint INTEGER NOT NULL DEFAULT 0,
    user_data_checkpoint INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_sync TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(user_id, device_id)
);

-- Indexes for items table
CREATE INDEX idx_items_library_id ON items(library_id);
CREATE INDEX idx_items_parent_id ON items(parent_id);
CREATE INDEX idx_items_item_kind ON items(item_kind);
CREATE INDEX idx_items_name ON items(name);
CREATE INDEX idx_items_production_year ON items(production_year);
CREATE INDEX idx_items_premiere_date ON items(premiere_date);
CREATE INDEX idx_items_date_created ON items(date_created);

-- Indexes for media_streams table
CREATE INDEX idx_media_streams_item_id ON media_streams(item_id);
CREATE INDEX idx_media_streams_stream_type ON media_streams(stream_type);

-- Indexes for images table
CREATE INDEX idx_images_item_id ON images(item_id);
CREATE INDEX idx_images_image_type ON images(image_type);

-- Indexes for user_item_data table
CREATE INDEX idx_user_item_data_user_id ON user_item_data(user_id);
CREATE INDEX idx_user_item_data_item_id ON user_item_data(item_id);

-- Indexes for auth_tokens table
CREATE INDEX idx_auth_tokens_user_id ON auth_tokens(user_id);
CREATE INDEX idx_auth_tokens_device_id ON auth_tokens(device_id);

-- Indexes for sync_change_log table
CREATE INDEX idx_sync_change_log_item_id ON sync_change_log(item_id);
CREATE INDEX idx_sync_change_log_changed_at ON sync_change_log(changed_at);
CREATE INDEX idx_sync_change_log_id_changed_at ON sync_change_log(id, changed_at);

-- Indexes for sync_user_data_log table
CREATE INDEX idx_sync_user_data_log_user_id ON sync_user_data_log(user_id);
CREATE INDEX idx_sync_user_data_log_item_id ON sync_user_data_log(item_id);
CREATE INDEX idx_sync_user_data_log_changed_at ON sync_user_data_log(changed_at);
CREATE INDEX idx_sync_user_data_log_id_changed_at ON sync_user_data_log(id, changed_at);

-- Indexes for sync_checkpoints table
CREATE INDEX idx_sync_checkpoints_user_id ON sync_checkpoints(user_id);
CREATE INDEX idx_sync_checkpoints_device_id ON sync_checkpoints(device_id);
