//! HLS cache queries.
//!
//! Stores and retrieves precomputed HLS streaming data (init segments,
//! segment maps with pre-built moof headers) to enable zero-parse serving.

use rusqlite::{params, Connection};
use sceneforged_common::{Error, MediaFileId, Result};
use uuid::Uuid;

/// Precomputed HLS data for a media file.
pub struct HlsCacheEntry {
    pub media_file_id: MediaFileId,
    pub init_segment: Vec<u8>,
    pub segment_count: u32,
    pub segment_map: Vec<u8>,
}

/// Store precomputed HLS data for a media file.
///
/// This should be called within the same transaction that sets Profile B,
/// ensuring the invariant: Profile B <-> HLS cache exists.
pub fn store(conn: &Connection, entry: &HlsCacheEntry) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO hls_cache (media_file_id, init_segment, segment_count, segment_map)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            entry.media_file_id.to_string(),
            entry.init_segment,
            entry.segment_count,
            entry.segment_map,
        ],
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(())
}

/// Get precomputed HLS data for a media file.
pub fn get(conn: &Connection, media_file_id: MediaFileId) -> Result<Option<HlsCacheEntry>> {
    match conn.query_row(
        "SELECT media_file_id, init_segment, segment_count, segment_map
         FROM hls_cache WHERE media_file_id = ?1",
        params![media_file_id.to_string()],
        |row| {
            Ok(HlsCacheEntry {
                media_file_id: MediaFileId::from(
                    Uuid::parse_str(&row.get::<_, String>(0)?).unwrap(),
                ),
                init_segment: row.get(1)?,
                segment_count: row.get::<_, u32>(2)?,
                segment_map: row.get(3)?,
            })
        },
    ) {
        Ok(entry) => Ok(Some(entry)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Get just the init segment for a media file (avoids loading full segment map).
pub fn get_init_segment(
    conn: &Connection,
    media_file_id: MediaFileId,
) -> Result<Option<Vec<u8>>> {
    match conn.query_row(
        "SELECT init_segment FROM hls_cache WHERE media_file_id = ?1",
        params![media_file_id.to_string()],
        |row| row.get::<_, Vec<u8>>(0),
    ) {
        Ok(data) => Ok(Some(data)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Delete HLS cache for a media file.
pub fn delete(conn: &Connection, media_file_id: MediaFileId) -> Result<bool> {
    let affected = conn
        .execute(
            "DELETE FROM hls_cache WHERE media_file_id = ?1",
            params![media_file_id.to_string()],
        )
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(affected > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::{init_memory_pool, PooledConnection};
    use crate::queries::media_files;
    use sceneforged_common::{FileRole, ItemId, LibraryId};

    fn setup_test_db() -> PooledConnection {
        let pool = init_memory_pool().unwrap();
        pool.get().unwrap()
    }

    fn create_test_item_and_file(conn: &Connection) -> (ItemId, MediaFileId) {
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

        let file = media_files::create_media_file(
            conn,
            item_id,
            FileRole::Source,
            "/media/movie.mkv",
            1024,
            "mkv",
        )
        .unwrap();

        (item_id, file.id)
    }

    #[test]
    fn test_store_and_get() {
        let conn = setup_test_db();
        let (_item_id, file_id) = create_test_item_and_file(&conn);

        let entry = HlsCacheEntry {
            media_file_id: file_id,
            init_segment: vec![0x00, 0x00, 0x00, 0x1C, 0x66, 0x74, 0x79, 0x70],
            segment_count: 42,
            segment_map: vec![1, 2, 3, 4, 5],
        };

        store(&conn, &entry).unwrap();

        let fetched = get(&conn, file_id).unwrap().unwrap();
        assert_eq!(fetched.media_file_id, file_id);
        assert_eq!(fetched.init_segment, entry.init_segment);
        assert_eq!(fetched.segment_count, 42);
        assert_eq!(fetched.segment_map, entry.segment_map);
    }

    #[test]
    fn test_get_nonexistent() {
        let conn = setup_test_db();
        let result = get(&conn, MediaFileId::new()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_init_segment() {
        let conn = setup_test_db();
        let (_item_id, file_id) = create_test_item_and_file(&conn);

        let init_data = vec![0x00, 0x00, 0x00, 0x1C, 0x66, 0x74, 0x79, 0x70];
        let entry = HlsCacheEntry {
            media_file_id: file_id,
            init_segment: init_data.clone(),
            segment_count: 10,
            segment_map: vec![0xFF; 1024],
        };

        store(&conn, &entry).unwrap();

        let fetched = get_init_segment(&conn, file_id).unwrap().unwrap();
        assert_eq!(fetched, init_data);
    }

    #[test]
    fn test_get_init_segment_nonexistent() {
        let conn = setup_test_db();
        let result = get_init_segment(&conn, MediaFileId::new()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_store_replaces_existing() {
        let conn = setup_test_db();
        let (_item_id, file_id) = create_test_item_and_file(&conn);

        let entry1 = HlsCacheEntry {
            media_file_id: file_id,
            init_segment: vec![1, 2, 3],
            segment_count: 10,
            segment_map: vec![4, 5, 6],
        };
        store(&conn, &entry1).unwrap();

        let entry2 = HlsCacheEntry {
            media_file_id: file_id,
            init_segment: vec![7, 8, 9],
            segment_count: 20,
            segment_map: vec![10, 11, 12],
        };
        store(&conn, &entry2).unwrap();

        let fetched = get(&conn, file_id).unwrap().unwrap();
        assert_eq!(fetched.init_segment, vec![7, 8, 9]);
        assert_eq!(fetched.segment_count, 20);
        assert_eq!(fetched.segment_map, vec![10, 11, 12]);
    }

    #[test]
    fn test_delete() {
        let conn = setup_test_db();
        let (_item_id, file_id) = create_test_item_and_file(&conn);

        let entry = HlsCacheEntry {
            media_file_id: file_id,
            init_segment: vec![1, 2, 3],
            segment_count: 5,
            segment_map: vec![4, 5, 6],
        };
        store(&conn, &entry).unwrap();

        assert!(delete(&conn, file_id).unwrap());
        assert!(get(&conn, file_id).unwrap().is_none());
    }

    #[test]
    fn test_delete_nonexistent() {
        let conn = setup_test_db();
        assert!(!delete(&conn, MediaFileId::new()).unwrap());
    }
}
