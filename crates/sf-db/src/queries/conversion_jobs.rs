//! Conversion job queue operations.

use chrono::Utc;
use rusqlite::Connection;
use sf_core::{ConversionJobId, Error, ItemId, MediaFileId, Result};

use crate::models::ConversionJob;

const COLS: &str = "id, item_id, media_file_id, status, progress_pct, encode_fps,
    eta_secs, error, created_at, started_at, completed_at,
    locked_by, locked_at, source_media_file_id, priority,
    bitrate, speed, output_size";

/// Create a new conversion job.
pub fn create_conversion_job(
    conn: &Connection,
    item_id: ItemId,
    source_media_file_id: MediaFileId,
) -> Result<ConversionJob> {
    let id = ConversionJobId::new();
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO conversion_jobs (id, item_id, source_media_file_id, status, created_at)
         VALUES (?1, ?2, ?3, 'queued', ?4)",
        rusqlite::params![id.to_string(), item_id.to_string(), source_media_file_id.to_string(), &now],
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(ConversionJob {
        id,
        item_id,
        media_file_id: None,
        status: "queued".to_string(),
        progress_pct: 0.0,
        encode_fps: None,
        eta_secs: None,
        error: None,
        created_at: now,
        started_at: None,
        completed_at: None,
        locked_by: None,
        locked_at: None,
        source_media_file_id: Some(source_media_file_id),
        priority: 0,
        bitrate: None,
        speed: None,
        output_size: None,
    })
}

/// Get a conversion job by ID.
pub fn get_conversion_job(
    conn: &Connection,
    id: ConversionJobId,
) -> Result<Option<ConversionJob>> {
    let q = format!("SELECT {COLS} FROM conversion_jobs WHERE id = ?1");
    let result = conn.query_row(&q, [id.to_string()], ConversionJob::from_row);
    match result {
        Ok(j) => Ok(Some(j)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// List conversion jobs with optional status filter and pagination.
pub fn list_conversion_jobs(
    conn: &Connection,
    status: Option<&str>,
    offset: i64,
    limit: i64,
) -> Result<Vec<ConversionJob>> {
    let (q, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(s) = status {
        (
            format!(
                "SELECT {COLS} FROM conversion_jobs WHERE status = ?1
                 ORDER BY priority DESC, created_at ASC LIMIT ?2 OFFSET ?3"
            ),
            vec![
                Box::new(s.to_string()),
                Box::new(limit),
                Box::new(offset),
            ],
        )
    } else {
        (
            format!(
                "SELECT {COLS} FROM conversion_jobs
                 ORDER BY priority DESC, created_at ASC LIMIT ?1 OFFSET ?2"
            ),
            vec![Box::new(limit), Box::new(offset)],
        )
    };

    let mut stmt = conn.prepare(&q).map_err(|e| Error::database(e.to_string()))?;
    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|b| b.as_ref()).collect();
    let rows = stmt
        .query_map(params_refs.as_slice(), ConversionJob::from_row)
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(rows)
}

/// Atomically dequeue the next queued conversion job.
///
/// Sets `status='processing'`, `locked_by`, `locked_at`, `started_at`.
pub fn dequeue_next_conversion(
    conn: &Connection,
    worker: &str,
) -> Result<Option<ConversionJob>> {
    let now = Utc::now().to_rfc3339();

    let q = format!(
        "UPDATE conversion_jobs SET status='processing', locked_by=?1, locked_at=?2, started_at=?2
         WHERE id = (
             SELECT id FROM conversion_jobs WHERE status='queued'
             ORDER BY priority DESC, created_at ASC LIMIT 1
         )
         RETURNING {COLS}"
    );

    let result = conn.query_row(&q, rusqlite::params![worker, &now], ConversionJob::from_row);
    match result {
        Ok(j) => Ok(Some(j)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Update conversion progress.
pub fn update_conversion_progress(
    conn: &Connection,
    id: ConversionJobId,
    pct: f64,
    fps: Option<f64>,
    eta: Option<i64>,
    bitrate: Option<&str>,
    speed: Option<&str>,
    output_size: Option<i64>,
) -> Result<bool> {
    let n = conn
        .execute(
            "UPDATE conversion_jobs SET progress_pct = ?1, encode_fps = ?2, eta_secs = ?3,
                bitrate = ?4, speed = ?5, output_size = ?6
             WHERE id = ?7",
            rusqlite::params![pct, fps, eta, bitrate, speed, output_size, id.to_string()],
        )
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

/// Mark a conversion job as completed, storing the output media_file_id.
pub fn complete_conversion(
    conn: &Connection,
    id: ConversionJobId,
    output_media_file_id: MediaFileId,
) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let n = conn
        .execute(
            "UPDATE conversion_jobs SET status='completed', progress_pct=100.0,
                media_file_id=?1, completed_at=?2
             WHERE id=?3",
            rusqlite::params![output_media_file_id.to_string(), now, id.to_string()],
        )
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

/// Mark a conversion job as failed.
pub fn fail_conversion(
    conn: &Connection,
    id: ConversionJobId,
    error: &str,
) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let n = conn
        .execute(
            "UPDATE conversion_jobs SET status='failed', error=?1, completed_at=?2 WHERE id=?3",
            rusqlite::params![error, now, id.to_string()],
        )
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

/// Delete a conversion job. Only queued or failed jobs can be deleted.
/// Returns true if a row was deleted.
pub fn delete_conversion_job(
    conn: &Connection,
    id: ConversionJobId,
) -> Result<bool> {
    let n = conn
        .execute(
            "DELETE FROM conversion_jobs WHERE id = ?1 AND status IN ('queued', 'failed')",
            [id.to_string()],
        )
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

/// Cancel a running conversion job by setting status to 'failed'.
pub fn cancel_conversion_job(
    conn: &Connection,
    id: ConversionJobId,
) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let n = conn
        .execute(
            "UPDATE conversion_jobs SET status='failed', error='Cancelled by user', completed_at=?1
             WHERE id = ?2 AND status IN ('queued', 'processing')",
            rusqlite::params![now, id.to_string()],
        )
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

/// Update the priority of a single conversion job.
pub fn update_conversion_priority(
    conn: &Connection,
    id: ConversionJobId,
    priority: i32,
) -> Result<bool> {
    let n = conn
        .execute(
            "UPDATE conversion_jobs SET priority = ?1 WHERE id = ?2",
            rusqlite::params![priority, id.to_string()],
        )
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

/// Reorder queued conversion jobs by setting priority = len-index for each.
/// First item in the list gets the highest priority.
pub fn reorder_queue(conn: &Connection, job_ids: &[ConversionJobId]) -> Result<()> {
    let len = job_ids.len() as i32;
    for (i, id) in job_ids.iter().enumerate() {
        let priority = len - i as i32;
        conn.execute(
            "UPDATE conversion_jobs SET priority = ?1 WHERE id = ?2 AND status = 'queued'",
            rusqlite::params![priority, id.to_string()],
        )
        .map_err(|e| Error::database(e.to_string()))?;
    }
    Ok(())
}

/// Check if an item already has a queued or processing conversion job.
pub fn has_active_conversion_for_item(
    conn: &Connection,
    item_id: ItemId,
) -> Result<bool> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversion_jobs WHERE item_id = ?1 AND status IN ('queued', 'processing')",
            [item_id.to_string()],
            |row| row.get(0),
        )
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(count > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_memory_pool;
    use crate::queries::{items, libraries, media_files};

    fn setup() -> (
        r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>,
        sf_core::ItemId,
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
            &conn, item.id, "/movie.mkv", "movie.mkv", 1024,
            Some("mkv"), Some("hevc"), Some("aac"),
            Some(1920), Some(1080), None, false, None,
            "source", "C", Some(7200.0),
        )
        .unwrap();
        (conn, item.id, mf.id)
    }

    #[test]
    fn create_and_get() {
        let (conn, item_id, mf_id) = setup();
        let job = create_conversion_job(&conn, item_id, mf_id).unwrap();
        assert_eq!(job.status, "queued");
        assert_eq!(job.source_media_file_id, Some(mf_id));

        let found = get_conversion_job(&conn, job.id).unwrap().unwrap();
        assert_eq!(found.item_id, item_id);
    }

    #[test]
    fn list_with_filter() {
        let (conn, item_id, mf_id) = setup();
        create_conversion_job(&conn, item_id, mf_id).unwrap();

        let all = list_conversion_jobs(&conn, None, 0, 100).unwrap();
        assert_eq!(all.len(), 1);

        let queued = list_conversion_jobs(&conn, Some("queued"), 0, 100).unwrap();
        assert_eq!(queued.len(), 1);

        let processing = list_conversion_jobs(&conn, Some("processing"), 0, 100).unwrap();
        assert!(processing.is_empty());
    }

    #[test]
    fn dequeue_and_complete() {
        let (conn, item_id, mf_id) = setup();
        create_conversion_job(&conn, item_id, mf_id).unwrap();

        let dequeued = dequeue_next_conversion(&conn, "w1").unwrap().unwrap();
        assert_eq!(dequeued.status, "processing");
        assert_eq!(dequeued.locked_by.as_deref(), Some("w1"));

        // Create an output media file for completion
        let out_mf = media_files::create_media_file(
            &conn, item_id, "/movie-pb.mp4", "movie-pb.mp4", 512,
            Some("mp4"), Some("h264"), Some("aac"),
            Some(1920), Some(1080), None, false, None,
            "universal", "B", Some(7200.0),
        )
        .unwrap();

        assert!(complete_conversion(&conn, dequeued.id, out_mf.id).unwrap());
        let done = get_conversion_job(&conn, dequeued.id).unwrap().unwrap();
        assert_eq!(done.status, "completed");
        assert_eq!(done.media_file_id, Some(out_mf.id));
    }

    #[test]
    fn fail_conversion_test() {
        let (conn, item_id, mf_id) = setup();
        let job = create_conversion_job(&conn, item_id, mf_id).unwrap();
        dequeue_next_conversion(&conn, "w1").unwrap();

        assert!(fail_conversion(&conn, job.id, "encode error").unwrap());
        let failed = get_conversion_job(&conn, job.id).unwrap().unwrap();
        assert_eq!(failed.status, "failed");
        assert_eq!(failed.error.as_deref(), Some("encode error"));
    }

    #[test]
    fn has_active_conversion() {
        let (conn, item_id, mf_id) = setup();
        assert!(!has_active_conversion_for_item(&conn, item_id).unwrap());

        create_conversion_job(&conn, item_id, mf_id).unwrap();
        assert!(has_active_conversion_for_item(&conn, item_id).unwrap());
    }

    #[test]
    fn progress_update() {
        let (conn, item_id, mf_id) = setup();
        let job = create_conversion_job(&conn, item_id, mf_id).unwrap();
        assert!(update_conversion_progress(
            &conn, job.id, 50.0, Some(24.5), Some(120),
            Some("5000kbits/s"), Some("1.5x"), Some(1024000),
        ).unwrap());

        let found = get_conversion_job(&conn, job.id).unwrap().unwrap();
        assert!((found.progress_pct - 50.0).abs() < f64::EPSILON);
        assert!((found.encode_fps.unwrap() - 24.5).abs() < f64::EPSILON);
        assert_eq!(found.eta_secs, Some(120));
        assert_eq!(found.bitrate.as_deref(), Some("5000kbits/s"));
        assert_eq!(found.speed.as_deref(), Some("1.5x"));
        assert_eq!(found.output_size, Some(1024000));
    }
}
