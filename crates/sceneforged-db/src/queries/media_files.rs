//! Media file query operations.
//!
//! This module provides CRUD operations for media files associated with library items.
//! Each item can have multiple files with different roles (source, universal, extra).

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use sceneforged_common::{Error, FileRole, ItemId, MediaFileId, Result};
use uuid::Uuid;

use crate::models::MediaFile;

/// Create a new media file for an item.
pub fn create_media_file(
    conn: &Connection,
    item_id: ItemId,
    role: FileRole,
    file_path: &str,
    file_size: i64,
    container: &str,
) -> Result<MediaFile> {
    let id = MediaFileId::new();
    let now = Utc::now();

    conn.execute(
        "INSERT INTO media_files (id, item_id, role, file_path, file_size, container, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        params![
            id.to_string(),
            item_id.to_string(),
            role.to_string(),
            file_path,
            file_size,
            container,
            now.to_rfc3339(),
        ],
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(MediaFile {
        id,
        item_id,
        role,
        file_path: file_path.to_string(),
        file_size,
        container: container.to_string(),
        video_codec: None,
        audio_codec: None,
        width: None,
        height: None,
        duration_ticks: None,
        bit_rate: None,
        is_hdr: false,
        serves_as_universal: false,
        has_faststart: false,
        keyframe_interval_secs: None,
        created_at: now,
    })
}

/// Get a media file by ID.
pub fn get_media_file(conn: &Connection, id: MediaFileId) -> Result<MediaFile> {
    conn.query_row(
        "SELECT id, item_id, role, file_path, file_size, container, video_codec, audio_codec,
                width, height, duration_ticks, bit_rate, is_hdr, serves_as_universal,
                has_faststart, keyframe_interval_secs, created_at
         FROM media_files WHERE id = ?",
        [id.to_string()],
        |row| {
            Ok(MediaFile {
                id: MediaFileId::from(Uuid::parse_str(&row.get::<_, String>(0)?).unwrap()),
                item_id: ItemId::from(Uuid::parse_str(&row.get::<_, String>(1)?).unwrap()),
                role: row.get::<_, String>(2)?.parse().unwrap_or(FileRole::Source),
                file_path: row.get(3)?,
                file_size: row.get(4)?,
                container: row.get(5)?,
                video_codec: row.get(6)?,
                audio_codec: row.get(7)?,
                width: row.get(8)?,
                height: row.get(9)?,
                duration_ticks: row.get(10)?,
                bit_rate: row.get(11)?,
                is_hdr: row.get::<_, i32>(12)? != 0,
                serves_as_universal: row.get::<_, i32>(13)? != 0,
                has_faststart: row.get::<_, i32>(14)? != 0,
                keyframe_interval_secs: row.get(15)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(16)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Error::not_found("media_file"),
        _ => Error::database(e.to_string()),
    })
}

/// Get a media file by item ID and role.
pub fn get_media_file_by_role(
    conn: &Connection,
    item_id: ItemId,
    role: FileRole,
) -> Result<Option<MediaFile>> {
    match conn.query_row(
        "SELECT id, item_id, role, file_path, file_size, container, video_codec, audio_codec,
                width, height, duration_ticks, bit_rate, is_hdr, serves_as_universal,
                has_faststart, keyframe_interval_secs, created_at
         FROM media_files WHERE item_id = ? AND role = ?",
        params![item_id.to_string(), role.to_string()],
        |row| {
            Ok(MediaFile {
                id: MediaFileId::from(Uuid::parse_str(&row.get::<_, String>(0)?).unwrap()),
                item_id: ItemId::from(Uuid::parse_str(&row.get::<_, String>(1)?).unwrap()),
                role: row.get::<_, String>(2)?.parse().unwrap_or(FileRole::Source),
                file_path: row.get(3)?,
                file_size: row.get(4)?,
                container: row.get(5)?,
                video_codec: row.get(6)?,
                audio_codec: row.get(7)?,
                width: row.get(8)?,
                height: row.get(9)?,
                duration_ticks: row.get(10)?,
                bit_rate: row.get(11)?,
                is_hdr: row.get::<_, i32>(12)? != 0,
                serves_as_universal: row.get::<_, i32>(13)? != 0,
                has_faststart: row.get::<_, i32>(14)? != 0,
                keyframe_interval_secs: row.get(15)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(16)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        },
    ) {
        Ok(file) => Ok(Some(file)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// List all media files for an item.
pub fn list_media_files_for_item(conn: &Connection, item_id: ItemId) -> Result<Vec<MediaFile>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, item_id, role, file_path, file_size, container, video_codec, audio_codec,
                    width, height, duration_ticks, bit_rate, is_hdr, serves_as_universal,
                    has_faststart, keyframe_interval_secs, created_at
             FROM media_files WHERE item_id = ? ORDER BY role",
        )
        .map_err(|e| Error::database(e.to_string()))?;

    let files = stmt
        .query_map([item_id.to_string()], |row| {
            Ok(MediaFile {
                id: MediaFileId::from(Uuid::parse_str(&row.get::<_, String>(0)?).unwrap()),
                item_id: ItemId::from(Uuid::parse_str(&row.get::<_, String>(1)?).unwrap()),
                role: row.get::<_, String>(2)?.parse().unwrap_or(FileRole::Source),
                file_path: row.get(3)?,
                file_size: row.get(4)?,
                container: row.get(5)?,
                video_codec: row.get(6)?,
                audio_codec: row.get(7)?,
                width: row.get(8)?,
                height: row.get(9)?,
                duration_ticks: row.get(10)?,
                bit_rate: row.get(11)?,
                is_hdr: row.get::<_, i32>(12)? != 0,
                serves_as_universal: row.get::<_, i32>(13)? != 0,
                has_faststart: row.get::<_, i32>(14)? != 0,
                keyframe_interval_secs: row.get(15)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(16)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(files)
}

/// Update media file metadata after probing.
#[allow(clippy::too_many_arguments)]
pub fn update_media_file_metadata(
    conn: &Connection,
    id: MediaFileId,
    video_codec: Option<&str>,
    audio_codec: Option<&str>,
    width: Option<i32>,
    height: Option<i32>,
    duration_ticks: Option<i64>,
    bit_rate: Option<i64>,
    is_hdr: bool,
    serves_as_universal: bool,
    has_faststart: bool,
    keyframe_interval_secs: Option<f64>,
) -> Result<()> {
    let affected = conn
        .execute(
            "UPDATE media_files SET
                video_codec = ?, audio_codec = ?, width = ?, height = ?,
                duration_ticks = ?, bit_rate = ?, is_hdr = ?, serves_as_universal = ?,
                has_faststart = ?, keyframe_interval_secs = ?
             WHERE id = ?",
            params![
                video_codec,
                audio_codec,
                width,
                height,
                duration_ticks,
                bit_rate,
                is_hdr as i32,
                serves_as_universal as i32,
                has_faststart as i32,
                keyframe_interval_secs,
                id.to_string(),
            ],
        )
        .map_err(|e| Error::database(e.to_string()))?;

    if affected == 0 {
        return Err(Error::not_found("media_file"));
    }

    Ok(())
}

/// Delete a media file.
pub fn delete_media_file(conn: &Connection, id: MediaFileId) -> Result<bool> {
    let affected = conn
        .execute("DELETE FROM media_files WHERE id = ?", [id.to_string()])
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(affected > 0)
}

/// Resolve which file to use for HLS streaming.
/// Returns universal file if exists, or source file if it serves as universal.
pub fn resolve_hls_file(conn: &Connection, item_id: ItemId) -> Result<Option<MediaFile>> {
    // First try universal file
    if let Some(file) = get_media_file_by_role(conn, item_id, FileRole::Universal)? {
        return Ok(Some(file));
    }

    // Fall back to source if it serves as universal
    if let Some(file) = get_media_file_by_role(conn, item_id, FileRole::Source)? {
        if file.serves_as_universal {
            return Ok(Some(file));
        }
    }

    Ok(None)
}

/// Resolve which file to use for direct streaming/download.
/// Prefers source (higher quality), falls back to universal.
pub fn resolve_direct_stream_file(conn: &Connection, item_id: ItemId) -> Result<Option<MediaFile>> {
    if let Some(file) = get_media_file_by_role(conn, item_id, FileRole::Source)? {
        return Ok(Some(file));
    }

    get_media_file_by_role(conn, item_id, FileRole::Universal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::{init_memory_pool, PooledConnection};

    fn setup_test_db() -> PooledConnection {
        let pool = init_memory_pool().unwrap();
        pool.get().unwrap()
    }

    fn create_test_item(conn: &Connection) -> ItemId {
        use sceneforged_common::LibraryId;

        let lib_id = LibraryId::new();
        conn.execute(
            "INSERT INTO libraries (id, name, media_type, paths) VALUES (?, ?, ?, ?)",
            params![lib_id.to_string(), "Movies", "movies", "[]"],
        )
        .unwrap();

        let item_id = ItemId::new();
        conn.execute(
            "INSERT INTO items (id, library_id, item_kind, name) VALUES (?, ?, ?, ?)",
            params![
                item_id.to_string(),
                lib_id.to_string(),
                "movie",
                "Test Movie"
            ],
        )
        .unwrap();

        item_id
    }

    #[test]
    fn test_create_media_file() {
        let conn = setup_test_db();
        let item_id = create_test_item(&conn);

        let file = create_media_file(
            &conn,
            item_id,
            FileRole::Source,
            "/media/movie.mkv",
            1024 * 1024 * 1024,
            "mkv",
        )
        .unwrap();

        assert_eq!(file.item_id, item_id);
        assert_eq!(file.role, FileRole::Source);
        assert_eq!(file.file_path, "/media/movie.mkv");
        assert_eq!(file.container, "mkv");
    }

    #[test]
    fn test_get_media_file() {
        let conn = setup_test_db();
        let item_id = create_test_item(&conn);

        let created = create_media_file(
            &conn,
            item_id,
            FileRole::Source,
            "/media/movie.mkv",
            1024,
            "mkv",
        )
        .unwrap();

        let fetched = get_media_file(&conn, created.id).unwrap();
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.file_path, "/media/movie.mkv");
    }

    #[test]
    fn test_get_media_file_by_role() {
        let conn = setup_test_db();
        let item_id = create_test_item(&conn);

        create_media_file(
            &conn,
            item_id,
            FileRole::Source,
            "/media/movie.mkv",
            1024,
            "mkv",
        )
        .unwrap();

        let source = get_media_file_by_role(&conn, item_id, FileRole::Source).unwrap();
        assert!(source.is_some());

        let universal = get_media_file_by_role(&conn, item_id, FileRole::Universal).unwrap();
        assert!(universal.is_none());
    }

    #[test]
    fn test_list_media_files() {
        let conn = setup_test_db();
        let item_id = create_test_item(&conn);

        create_media_file(
            &conn,
            item_id,
            FileRole::Source,
            "/media/movie.mkv",
            1024,
            "mkv",
        )
        .unwrap();
        create_media_file(
            &conn,
            item_id,
            FileRole::Universal,
            "/cache/movie.mp4",
            512,
            "mp4",
        )
        .unwrap();

        let files = list_media_files_for_item(&conn, item_id).unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_update_media_file_metadata() {
        let conn = setup_test_db();
        let item_id = create_test_item(&conn);

        let file = create_media_file(
            &conn,
            item_id,
            FileRole::Source,
            "/media/movie.mkv",
            1024,
            "mkv",
        )
        .unwrap();

        update_media_file_metadata(
            &conn,
            file.id,
            Some("hevc"),
            Some("aac"),
            Some(1920),
            Some(1080),
            Some(72000000000),
            Some(5000000),
            true,
            false,
            false,
            Some(2.5),
        )
        .unwrap();

        let updated = get_media_file(&conn, file.id).unwrap();
        assert_eq!(updated.video_codec, Some("hevc".to_string()));
        assert_eq!(updated.width, Some(1920));
        assert!(updated.is_hdr);
    }

    #[test]
    fn test_resolve_hls_file() {
        let conn = setup_test_db();
        let item_id = create_test_item(&conn);

        // No files - should return None
        assert!(resolve_hls_file(&conn, item_id).unwrap().is_none());

        // Add source that doesn't serve as universal
        let source = create_media_file(
            &conn,
            item_id,
            FileRole::Source,
            "/media/movie.mkv",
            1024,
            "mkv",
        )
        .unwrap();
        assert!(resolve_hls_file(&conn, item_id).unwrap().is_none());

        // Mark source as serving as universal
        update_media_file_metadata(
            &conn,
            source.id,
            None,
            None,
            None,
            None,
            None,
            None,
            false,
            true,
            true,
            Some(2.0),
        )
        .unwrap();
        let resolved = resolve_hls_file(&conn, item_id).unwrap();
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().role, FileRole::Source);

        // Add universal file - should prefer it
        create_media_file(
            &conn,
            item_id,
            FileRole::Universal,
            "/cache/movie.mp4",
            512,
            "mp4",
        )
        .unwrap();
        let resolved = resolve_hls_file(&conn, item_id).unwrap();
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().role, FileRole::Universal);
    }

    #[test]
    fn test_delete_media_file() {
        let conn = setup_test_db();
        let item_id = create_test_item(&conn);

        let file = create_media_file(
            &conn,
            item_id,
            FileRole::Source,
            "/media/movie.mkv",
            1024,
            "mkv",
        )
        .unwrap();
        assert!(delete_media_file(&conn, file.id).unwrap());
        assert!(get_media_file(&conn, file.id).is_err());
    }
}
