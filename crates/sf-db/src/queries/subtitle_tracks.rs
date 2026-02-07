//! Subtitle track CRUD operations.

use chrono::Utc;
use rusqlite::Connection;
use sf_core::{Error, MediaFileId, Result, SubtitleTrackId};

use crate::models::SubtitleTrack;

const COLS: &str = "id, media_file_id, track_index, codec, language, forced, default_track, created_at";

/// Create a new subtitle track record.
pub fn create_subtitle_track(
    conn: &Connection,
    media_file_id: MediaFileId,
    track_index: i32,
    codec: &str,
    language: Option<&str>,
    forced: bool,
    default_track: bool,
) -> Result<SubtitleTrack> {
    let id = SubtitleTrackId::new();
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO subtitle_tracks (id, media_file_id, track_index, codec, language, forced, default_track, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        rusqlite::params![
            id.to_string(),
            media_file_id.to_string(),
            track_index,
            codec,
            language,
            forced as i32,
            default_track as i32,
            &now,
        ],
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(SubtitleTrack {
        id,
        media_file_id,
        track_index,
        codec: codec.to_string(),
        language: language.map(String::from),
        forced,
        default_track,
        created_at: now,
    })
}

/// List subtitle tracks for a media file.
pub fn list_by_media_file(conn: &Connection, media_file_id: MediaFileId) -> Result<Vec<SubtitleTrack>> {
    let q = format!(
        "SELECT {COLS} FROM subtitle_tracks WHERE media_file_id = ?1 ORDER BY track_index ASC"
    );
    let mut stmt = conn.prepare(&q).map_err(|e| Error::database(e.to_string()))?;
    let rows = stmt
        .query_map([media_file_id.to_string()], SubtitleTrack::from_row)
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(rows)
}

/// Delete all subtitle tracks for a media file.
pub fn delete_by_media_file(conn: &Connection, media_file_id: MediaFileId) -> Result<usize> {
    let n = conn
        .execute(
            "DELETE FROM subtitle_tracks WHERE media_file_id = ?1",
            [media_file_id.to_string()],
        )
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_memory_pool;
    use crate::queries::{items, libraries, media_files};

    fn setup() -> (r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>, MediaFileId) {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let lib = libraries::create_library(&conn, "M", "movies", &[], &serde_json::json!({})).unwrap();
        let item = items::create_item(
            &conn, lib.id, "movie", "T", None, None, None, None, None, None, None, None, None,
        )
        .unwrap();
        let mf = media_files::create_media_file(
            &conn, item.id, "/test.mkv", "test.mkv", 1000,
            Some("mkv"), Some("h264"), Some("aac"),
            Some(1920), Some(1080), None, false, None, "source", "A", None,
        )
        .unwrap();
        (conn, mf.id)
    }

    #[test]
    fn create_and_list() {
        let (conn, mf_id) = setup();
        create_subtitle_track(&conn, mf_id, 0, "SRT", Some("eng"), false, true).unwrap();
        create_subtitle_track(&conn, mf_id, 1, "ASS", Some("jpn"), false, false).unwrap();

        let tracks = list_by_media_file(&conn, mf_id).unwrap();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].codec, "SRT");
        assert_eq!(tracks[0].language.as_deref(), Some("eng"));
        assert!(tracks[0].default_track);
        assert_eq!(tracks[1].track_index, 1);
    }

    #[test]
    fn delete_by_media() {
        let (conn, mf_id) = setup();
        create_subtitle_track(&conn, mf_id, 0, "SRT", Some("eng"), false, false).unwrap();
        assert_eq!(delete_by_media_file(&conn, mf_id).unwrap(), 1);
        assert!(list_by_media_file(&conn, mf_id).unwrap().is_empty());
    }
}
