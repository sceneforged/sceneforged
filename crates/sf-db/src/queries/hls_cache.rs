//! HLS cache operations.

use chrono::Utc;
use rusqlite::Connection;
use sf_core::{Error, MediaFileId, Result};

use crate::models::HlsCache;

const COLS: &str = "media_file_id, playlist, segments, created_at";

/// Insert or update an HLS cache entry.
///
/// `playlist` stores the `.hls/` directory path.
/// `segments` is unused for now but kept for future byte-range maps.
pub fn upsert_hls_cache(
    conn: &Connection,
    media_file_id: MediaFileId,
    hls_dir_path: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO hls_cache (media_file_id, playlist, segments, created_at)
         VALUES (?1, ?2, '[]', ?3)
         ON CONFLICT(media_file_id)
         DO UPDATE SET playlist = excluded.playlist, created_at = excluded.created_at",
        rusqlite::params![media_file_id.to_string(), hls_dir_path, now],
    )
    .map_err(|e| Error::database(e.to_string()))?;
    Ok(())
}

/// Get the HLS cache entry for a media file.
pub fn get_hls_cache(
    conn: &Connection,
    media_file_id: MediaFileId,
) -> Result<Option<HlsCache>> {
    let q = format!("SELECT {COLS} FROM hls_cache WHERE media_file_id = ?1");
    let result = conn.query_row(&q, [media_file_id.to_string()], HlsCache::from_row);
    match result {
        Ok(c) => Ok(Some(c)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Delete an HLS cache entry.
pub fn delete_hls_cache(
    conn: &Connection,
    media_file_id: MediaFileId,
) -> Result<bool> {
    let n = conn
        .execute(
            "DELETE FROM hls_cache WHERE media_file_id = ?1",
            [media_file_id.to_string()],
        )
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_memory_pool;
    use crate::queries::{items, libraries, media_files};

    fn setup() -> (
        r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>,
        sf_core::MediaFileId,
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
        let mf = media_files::create_media_file(
            &conn, item.id, "/movie-pb.mp4", "movie-pb.mp4", 512,
            Some("mp4"), Some("h264"), Some("aac"),
            Some(1920), Some(1080), None, false, None,
            "universal", "B", Some(7200.0),
        )
        .unwrap();
        (conn, mf.id)
    }

    #[test]
    fn upsert_and_get() {
        let (conn, mf_id) = setup();
        upsert_hls_cache(&conn, mf_id, "/data/movie/.hls").unwrap();

        let cache = get_hls_cache(&conn, mf_id).unwrap().unwrap();
        assert_eq!(cache.playlist, "/data/movie/.hls");
    }

    #[test]
    fn upsert_overwrites() {
        let (conn, mf_id) = setup();
        upsert_hls_cache(&conn, mf_id, "/old/.hls").unwrap();
        upsert_hls_cache(&conn, mf_id, "/new/.hls").unwrap();

        let cache = get_hls_cache(&conn, mf_id).unwrap().unwrap();
        assert_eq!(cache.playlist, "/new/.hls");
    }

    #[test]
    fn delete_cache() {
        let (conn, mf_id) = setup();
        upsert_hls_cache(&conn, mf_id, "/data/.hls").unwrap();
        assert!(delete_hls_cache(&conn, mf_id).unwrap());
        assert!(get_hls_cache(&conn, mf_id).unwrap().is_none());
    }
}
