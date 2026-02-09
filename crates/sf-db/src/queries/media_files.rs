//! Media-file CRUD operations.

use chrono::Utc;
use rusqlite::Connection;
use sf_core::{Error, ItemId, MediaFileId, Result};

use crate::models::MediaFile;

const COLS: &str = "id, item_id, file_path, file_name, file_size, container,
    video_codec, audio_codec, resolution_width, resolution_height,
    hdr_format, has_dolby_vision, dv_profile, role, profile,
    duration_secs, created_at, (hls_prepared IS NOT NULL)";

/// Create a new media file record.
#[allow(clippy::too_many_arguments)]
pub fn create_media_file(
    conn: &Connection,
    item_id: ItemId,
    file_path: &str,
    file_name: &str,
    file_size: i64,
    container: Option<&str>,
    video_codec: Option<&str>,
    audio_codec: Option<&str>,
    resolution_width: Option<i32>,
    resolution_height: Option<i32>,
    hdr_format: Option<&str>,
    has_dolby_vision: bool,
    dv_profile: Option<i32>,
    role: &str,
    profile: &str,
    duration_secs: Option<f64>,
) -> Result<MediaFile> {
    create_media_file_with_hls(
        conn, item_id, file_path, file_name, file_size,
        container, video_codec, audio_codec, resolution_width, resolution_height,
        hdr_format, has_dolby_vision, dv_profile, role, profile, duration_secs, None,
    )
}

/// Create a new media file record with optional pre-computed HLS data.
#[allow(clippy::too_many_arguments)]
pub fn create_media_file_with_hls(
    conn: &Connection,
    item_id: ItemId,
    file_path: &str,
    file_name: &str,
    file_size: i64,
    container: Option<&str>,
    video_codec: Option<&str>,
    audio_codec: Option<&str>,
    resolution_width: Option<i32>,
    resolution_height: Option<i32>,
    hdr_format: Option<&str>,
    has_dolby_vision: bool,
    dv_profile: Option<i32>,
    role: &str,
    profile: &str,
    duration_secs: Option<f64>,
    hls_prepared: Option<&[u8]>,
) -> Result<MediaFile> {
    let id = MediaFileId::new();
    let created_at = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO media_files (id, item_id, file_path, file_name, file_size,
            container, video_codec, audio_codec, resolution_width, resolution_height,
            hdr_format, has_dolby_vision, dv_profile, role, profile, duration_secs,
            created_at, hls_prepared)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
        rusqlite::params![
            id.to_string(),
            item_id.to_string(),
            file_path,
            file_name,
            file_size,
            container,
            video_codec,
            audio_codec,
            resolution_width,
            resolution_height,
            hdr_format,
            has_dolby_vision as i32,
            dv_profile,
            role,
            profile,
            duration_secs,
            created_at,
            hls_prepared,
        ],
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(MediaFile {
        id,
        item_id,
        file_path: file_path.to_string(),
        file_name: file_name.to_string(),
        file_size,
        container: container.map(String::from),
        video_codec: video_codec.map(String::from),
        audio_codec: audio_codec.map(String::from),
        resolution_width,
        resolution_height,
        hdr_format: hdr_format.map(String::from),
        has_dolby_vision,
        dv_profile,
        role: role.to_string(),
        profile: profile.to_string(),
        duration_secs,
        created_at,
        hls_ready: hls_prepared.is_some(),
    })
}

/// Store the serialized HLS PreparedMedia blob for a media file.
pub fn set_hls_prepared(conn: &Connection, id: MediaFileId, data: &[u8]) -> Result<()> {
    conn.execute(
        "UPDATE media_files SET hls_prepared = ?1 WHERE id = ?2",
        rusqlite::params![data, id.to_string()],
    )
    .map_err(|e| Error::database(e.to_string()))?;
    Ok(())
}

/// Load the serialized HLS PreparedMedia blob for a media file.
pub fn get_hls_prepared(conn: &Connection, id: MediaFileId) -> Result<Option<Vec<u8>>> {
    let result = conn.query_row(
        "SELECT hls_prepared FROM media_files WHERE id = ?1",
        [id.to_string()],
        |row| row.get::<_, Option<Vec<u8>>>(0),
    );
    match result {
        Ok(blob) => Ok(blob),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Get a media file by ID.
pub fn get_media_file(conn: &Connection, id: MediaFileId) -> Result<Option<MediaFile>> {
    let q = format!("SELECT {COLS} FROM media_files WHERE id = ?1");
    let result = conn.query_row(&q, [id.to_string()], MediaFile::from_row);
    match result {
        Ok(mf) => Ok(Some(mf)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// List media files belonging to an item.
pub fn list_media_files_by_item(conn: &Connection, item_id: ItemId) -> Result<Vec<MediaFile>> {
    let q = format!("SELECT {COLS} FROM media_files WHERE item_id = ?1 ORDER BY created_at ASC");
    let mut stmt = conn.prepare(&q).map_err(|e| Error::database(e.to_string()))?;
    let rows = stmt
        .query_map([item_id.to_string()], MediaFile::from_row)
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(rows)
}

/// Delete a media file by ID.
pub fn delete_media_file(conn: &Connection, id: MediaFileId) -> Result<bool> {
    let n = conn
        .execute("DELETE FROM media_files WHERE id = ?1", [id.to_string()])
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

/// Get a media file by its unique file_path.
pub fn get_media_file_by_path(conn: &Connection, path: &str) -> Result<Option<MediaFile>> {
    let q = format!("SELECT {COLS} FROM media_files WHERE file_path = ?1");
    let result = conn.query_row(&q, [path], MediaFile::from_row);
    match result {
        Ok(mf) => Ok(Some(mf)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// List all media files with a given profile (e.g. "B").
pub fn list_media_files_by_profile(conn: &Connection, profile: &str) -> Result<Vec<MediaFile>> {
    let q = format!(
        "SELECT {COLS} FROM media_files WHERE profile = ?1 ORDER BY created_at ASC"
    );
    let mut stmt = conn.prepare(&q).map_err(|e| Error::database(e.to_string()))?;
    let rows = stmt
        .query_map([profile], MediaFile::from_row)
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(rows)
}

/// Count distinct items that have at least one media file with each profile.
///
/// Returns a list of `(profile, count)` pairs.
pub fn count_items_by_profile(conn: &Connection) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn
        .prepare(
            "SELECT profile, COUNT(DISTINCT item_id) FROM media_files GROUP BY profile ORDER BY profile",
        )
        .map_err(|e| Error::database(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(rows)
}

/// Get total storage used by all media files.
pub fn total_storage_bytes(conn: &Connection) -> Result<i64> {
    conn.query_row(
        "SELECT COALESCE(SUM(file_size), 0) FROM media_files",
        [],
        |row| row.get(0),
    )
    .map_err(|e| Error::database(e.to_string()))
}

/// Count total media files.
pub fn count_media_files(conn: &Connection) -> Result<i64> {
    conn.query_row("SELECT COUNT(*) FROM media_files", [], |row| row.get(0))
        .map_err(|e| Error::database(e.to_string()))
}

/// List all media file paths for a library (used for batch existence checks during scan).
pub fn list_media_file_paths_for_library(
    conn: &Connection,
    library_id: sf_core::LibraryId,
) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare(
            "SELECT mf.file_path FROM media_files mf
             JOIN items i ON mf.item_id = i.id
             WHERE i.library_id = ?1",
        )
        .map_err(|e| Error::database(e.to_string()))?;
    let rows = stmt
        .query_map([library_id.to_string()], |row| row.get::<_, String>(0))
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_memory_pool;
    use crate::queries::{items, libraries};

    fn setup() -> (
        r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>,
        sf_core::ItemId,
    ) {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let lib = libraries::create_library(
            &conn,
            "Movies",
            "movies",
            &[],
            &serde_json::json!({}),
        )
        .unwrap();
        let item = items::create_item(
            &conn,
            lib.id,
            "movie",
            "Test",
            None, None, None, None, None, None, None, None, None,
        )
        .unwrap();
        (conn, item.id)
    }

    #[test]
    fn create_and_get() {
        let (conn, item_id) = setup();
        let mf = create_media_file(
            &conn, item_id, "/movie.mkv", "movie.mkv", 1024,
            Some("mkv"), Some("hevc"), Some("aac"),
            Some(1920), Some(1080), None, false, None,
            "source", "C", Some(7200.0),
        )
        .unwrap();
        let found = get_media_file(&conn, mf.id).unwrap().unwrap();
        assert_eq!(found.file_path, "/movie.mkv");
    }

    #[test]
    fn list_and_delete() {
        let (conn, item_id) = setup();
        let mf = create_media_file(
            &conn, item_id, "/a.mkv", "a.mkv", 100,
            None, None, None, None, None, None, false, None,
            "source", "C", None,
        )
        .unwrap();
        let list = list_media_files_by_item(&conn, item_id).unwrap();
        assert_eq!(list.len(), 1);

        assert!(delete_media_file(&conn, mf.id).unwrap());
        assert!(list_media_files_by_item(&conn, item_id).unwrap().is_empty());
    }

    #[test]
    fn get_by_path() {
        let (conn, item_id) = setup();
        create_media_file(
            &conn, item_id, "/unique.mkv", "unique.mkv", 50,
            None, None, None, None, None, None, false, None,
            "source", "C", None,
        )
        .unwrap();
        let found = get_media_file_by_path(&conn, "/unique.mkv").unwrap();
        assert!(found.is_some());
    }
}
