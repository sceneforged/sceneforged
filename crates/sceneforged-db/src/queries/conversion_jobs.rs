//! Conversion job query operations.
//!
//! This module provides CRUD operations for conversion jobs that track
//! the transcoding of source files to universal format.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use sceneforged_common::{Error, ItemId, MediaFileId, Result};
use uuid::Uuid;

use crate::models::{ConversionJob, ConversionStatus};

/// Create a new conversion job.
pub fn create_conversion_job(
    conn: &Connection,
    item_id: ItemId,
    source_file_id: MediaFileId,
) -> Result<ConversionJob> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();

    conn.execute(
        "INSERT INTO conversion_jobs (id, item_id, source_file_id, status, progress_pct, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
        params![
            &id,
            item_id.to_string(),
            source_file_id.to_string(),
            "queued",
            0.0,
            now.to_rfc3339(),
        ],
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(ConversionJob {
        id,
        item_id,
        source_file_id,
        status: ConversionStatus::Queued,
        progress_pct: 0.0,
        output_path: None,
        error_message: None,
        hw_accel_used: None,
        encode_fps: None,
        started_at: None,
        completed_at: None,
        created_at: now,
    })
}

/// Get a conversion job by ID.
pub fn get_conversion_job(conn: &Connection, id: &str) -> Result<ConversionJob> {
    conn.query_row(
        "SELECT id, item_id, source_file_id, status, progress_pct, output_path, error_message,
                hw_accel_used, encode_fps, started_at, completed_at, created_at
         FROM conversion_jobs WHERE id = ?",
        [id],
        |row| {
            Ok(ConversionJob {
                id: row.get(0)?,
                item_id: ItemId::from(Uuid::parse_str(&row.get::<_, String>(1)?).unwrap()),
                source_file_id: MediaFileId::from(
                    Uuid::parse_str(&row.get::<_, String>(2)?).unwrap(),
                ),
                status: row
                    .get::<_, String>(3)?
                    .parse()
                    .unwrap_or(ConversionStatus::Queued),
                progress_pct: row.get(4)?,
                output_path: row.get(5)?,
                error_message: row.get(6)?,
                hw_accel_used: row.get(7)?,
                encode_fps: row.get(8)?,
                started_at: row
                    .get::<_, Option<String>>(9)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                completed_at: row
                    .get::<_, Option<String>>(10)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(11)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Error::not_found("conversion_job"),
        _ => Error::database(e.to_string()),
    })
}

/// Get the active conversion job for an item (if any).
pub fn get_active_job_for_item(
    conn: &Connection,
    item_id: ItemId,
) -> Result<Option<ConversionJob>> {
    match conn.query_row(
        "SELECT id, item_id, source_file_id, status, progress_pct, output_path, error_message,
                hw_accel_used, encode_fps, started_at, completed_at, created_at
         FROM conversion_jobs
         WHERE item_id = ? AND status IN ('queued', 'running')
         ORDER BY created_at DESC LIMIT 1",
        [item_id.to_string()],
        |row| {
            Ok(ConversionJob {
                id: row.get(0)?,
                item_id: ItemId::from(Uuid::parse_str(&row.get::<_, String>(1)?).unwrap()),
                source_file_id: MediaFileId::from(
                    Uuid::parse_str(&row.get::<_, String>(2)?).unwrap(),
                ),
                status: row
                    .get::<_, String>(3)?
                    .parse()
                    .unwrap_or(ConversionStatus::Queued),
                progress_pct: row.get(4)?,
                output_path: row.get(5)?,
                error_message: row.get(6)?,
                hw_accel_used: row.get(7)?,
                encode_fps: row.get(8)?,
                started_at: row
                    .get::<_, Option<String>>(9)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                completed_at: row
                    .get::<_, Option<String>>(10)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(11)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        },
    ) {
        Ok(job) => Ok(Some(job)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// List conversion jobs by status.
pub fn list_jobs_by_status(
    conn: &Connection,
    status: ConversionStatus,
    limit: usize,
) -> Result<Vec<ConversionJob>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, item_id, source_file_id, status, progress_pct, output_path, error_message,
                    hw_accel_used, encode_fps, started_at, completed_at, created_at
             FROM conversion_jobs
             WHERE status = ?
             ORDER BY created_at ASC
             LIMIT ?",
        )
        .map_err(|e| Error::database(e.to_string()))?;

    let jobs = stmt
        .query_map(params![status.to_string(), limit as i64], |row| {
            Ok(ConversionJob {
                id: row.get(0)?,
                item_id: ItemId::from(Uuid::parse_str(&row.get::<_, String>(1)?).unwrap()),
                source_file_id: MediaFileId::from(
                    Uuid::parse_str(&row.get::<_, String>(2)?).unwrap(),
                ),
                status: row
                    .get::<_, String>(3)?
                    .parse()
                    .unwrap_or(ConversionStatus::Queued),
                progress_pct: row.get(4)?,
                output_path: row.get(5)?,
                error_message: row.get(6)?,
                hw_accel_used: row.get(7)?,
                encode_fps: row.get(8)?,
                started_at: row
                    .get::<_, Option<String>>(9)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                completed_at: row
                    .get::<_, Option<String>>(10)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(11)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(jobs)
}

/// Get the next queued job (FIFO).
pub fn dequeue_next_job(conn: &Connection) -> Result<Option<ConversionJob>> {
    let jobs = list_jobs_by_status(conn, ConversionStatus::Queued, 1)?;
    Ok(jobs.into_iter().next())
}

/// Update job status to running.
pub fn start_job(conn: &Connection, id: &str, hw_accel: Option<&str>) -> Result<()> {
    let now = Utc::now();
    let affected = conn
        .execute(
            "UPDATE conversion_jobs SET status = 'running', started_at = ?, hw_accel_used = ?
             WHERE id = ? AND status = 'queued'",
            params![now.to_rfc3339(), hw_accel, id],
        )
        .map_err(|e| Error::database(e.to_string()))?;

    if affected == 0 {
        return Err(Error::not_found("conversion_job"));
    }

    Ok(())
}

/// Update job progress.
pub fn update_progress(
    conn: &Connection,
    id: &str,
    progress_pct: f64,
    encode_fps: Option<f64>,
) -> Result<()> {
    let affected = conn
        .execute(
            "UPDATE conversion_jobs SET progress_pct = ?, encode_fps = ?
             WHERE id = ? AND status = 'running'",
            params![progress_pct, encode_fps, id],
        )
        .map_err(|e| Error::database(e.to_string()))?;

    if affected == 0 {
        return Err(Error::not_found("conversion_job"));
    }

    Ok(())
}

/// Complete a job successfully.
pub fn complete_job(conn: &Connection, id: &str, output_path: &str) -> Result<()> {
    let now = Utc::now();
    let affected = conn
        .execute(
            "UPDATE conversion_jobs SET status = 'completed', progress_pct = 100.0,
             output_path = ?, completed_at = ?
             WHERE id = ? AND status = 'running'",
            params![output_path, now.to_rfc3339(), id],
        )
        .map_err(|e| Error::database(e.to_string()))?;

    if affected == 0 {
        return Err(Error::not_found("conversion_job"));
    }

    Ok(())
}

/// Fail a job with an error message.
pub fn fail_job(conn: &Connection, id: &str, error_message: &str) -> Result<()> {
    let now = Utc::now();
    let affected = conn
        .execute(
            "UPDATE conversion_jobs SET status = 'failed', error_message = ?, completed_at = ?
             WHERE id = ? AND status IN ('queued', 'running')",
            params![error_message, now.to_rfc3339(), id],
        )
        .map_err(|e| Error::database(e.to_string()))?;

    if affected == 0 {
        return Err(Error::not_found("conversion_job"));
    }

    Ok(())
}

/// Cancel a queued or running job.
pub fn cancel_job(conn: &Connection, id: &str) -> Result<()> {
    let now = Utc::now();
    let affected = conn
        .execute(
            "UPDATE conversion_jobs SET status = 'cancelled', completed_at = ?
             WHERE id = ? AND status IN ('queued', 'running')",
            params![now.to_rfc3339(), id],
        )
        .map_err(|e| Error::database(e.to_string()))?;

    if affected == 0 {
        return Err(Error::not_found("conversion_job"));
    }

    Ok(())
}

/// Delete old completed/failed/cancelled jobs.
pub fn prune_old_jobs(conn: &Connection, days: i32) -> Result<usize> {
    let affected = conn
        .execute(
            "DELETE FROM conversion_jobs
             WHERE status IN ('completed', 'failed', 'cancelled')
             AND completed_at < datetime('now', ? || ' days')",
            params![format!("-{}", days)],
        )
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(affected)
}

/// List all active (queued or running) conversion jobs.
pub fn list_active_jobs(conn: &Connection, limit: usize) -> Result<Vec<ConversionJob>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, item_id, source_file_id, status, progress_pct, output_path, error_message,
                    hw_accel_used, encode_fps, started_at, completed_at, created_at
             FROM conversion_jobs
             WHERE status IN ('queued', 'running')
             ORDER BY created_at ASC
             LIMIT ?",
        )
        .map_err(|e| Error::database(e.to_string()))?;

    let jobs = stmt
        .query_map(params![limit as i64], |row| {
            Ok(ConversionJob {
                id: row.get(0)?,
                item_id: ItemId::from(Uuid::parse_str(&row.get::<_, String>(1)?).unwrap()),
                source_file_id: MediaFileId::from(
                    Uuid::parse_str(&row.get::<_, String>(2)?).unwrap(),
                ),
                status: row
                    .get::<_, String>(3)?
                    .parse()
                    .unwrap_or(ConversionStatus::Queued),
                progress_pct: row.get(4)?,
                output_path: row.get(5)?,
                error_message: row.get(6)?,
                hw_accel_used: row.get(7)?,
                encode_fps: row.get(8)?,
                started_at: row
                    .get::<_, Option<String>>(9)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                completed_at: row
                    .get::<_, Option<String>>(10)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(11)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(jobs)
}

/// Cancel all stale jobs (running jobs that started more than `hours` ago).
/// Returns the number of jobs cancelled.
pub fn cancel_stale_jobs(conn: &Connection, hours: i32) -> Result<usize> {
    let now = Utc::now();
    let affected = conn
        .execute(
            "UPDATE conversion_jobs
             SET status = 'failed',
                 error_message = 'Job timed out (stale)',
                 completed_at = ?
             WHERE status = 'running'
             AND started_at < datetime('now', ? || ' hours')",
            params![now.to_rfc3339(), format!("-{}", hours)],
        )
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(affected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::{init_memory_pool, PooledConnection};
    use crate::queries::media_files;
    use sceneforged_common::{FileRole, LibraryId};

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
    fn test_create_conversion_job() {
        let conn = setup_test_db();
        let (item_id, file_id) = create_test_item_and_file(&conn);

        let job = create_conversion_job(&conn, item_id, file_id).unwrap();
        assert_eq!(job.item_id, item_id);
        assert_eq!(job.source_file_id, file_id);
        assert_eq!(job.status, ConversionStatus::Queued);
        assert_eq!(job.progress_pct, 0.0);
    }

    #[test]
    fn test_get_conversion_job() {
        let conn = setup_test_db();
        let (item_id, file_id) = create_test_item_and_file(&conn);

        let created = create_conversion_job(&conn, item_id, file_id).unwrap();
        let fetched = get_conversion_job(&conn, &created.id).unwrap();

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.status, ConversionStatus::Queued);
    }

    #[test]
    fn test_job_lifecycle() {
        let conn = setup_test_db();
        let (item_id, file_id) = create_test_item_and_file(&conn);

        let job = create_conversion_job(&conn, item_id, file_id).unwrap();

        // Start job
        start_job(&conn, &job.id, Some("nvenc")).unwrap();
        let job = get_conversion_job(&conn, &job.id).unwrap();
        assert_eq!(job.status, ConversionStatus::Running);
        assert!(job.started_at.is_some());
        assert_eq!(job.hw_accel_used, Some("nvenc".to_string()));

        // Update progress
        update_progress(&conn, &job.id, 50.0, Some(120.5)).unwrap();
        let job = get_conversion_job(&conn, &job.id).unwrap();
        assert_eq!(job.progress_pct, 50.0);
        assert_eq!(job.encode_fps, Some(120.5));

        // Complete job
        complete_job(&conn, &job.id, "/cache/movie.mp4").unwrap();
        let job = get_conversion_job(&conn, &job.id).unwrap();
        assert_eq!(job.status, ConversionStatus::Completed);
        assert_eq!(job.progress_pct, 100.0);
        assert_eq!(job.output_path, Some("/cache/movie.mp4".to_string()));
        assert!(job.completed_at.is_some());
    }

    #[test]
    fn test_fail_job() {
        let conn = setup_test_db();
        let (item_id, file_id) = create_test_item_and_file(&conn);

        let job = create_conversion_job(&conn, item_id, file_id).unwrap();
        start_job(&conn, &job.id, None).unwrap();
        fail_job(&conn, &job.id, "Encoder error").unwrap();

        let job = get_conversion_job(&conn, &job.id).unwrap();
        assert_eq!(job.status, ConversionStatus::Failed);
        assert_eq!(job.error_message, Some("Encoder error".to_string()));
    }

    #[test]
    fn test_cancel_job() {
        let conn = setup_test_db();
        let (item_id, file_id) = create_test_item_and_file(&conn);

        let job = create_conversion_job(&conn, item_id, file_id).unwrap();
        cancel_job(&conn, &job.id).unwrap();

        let job = get_conversion_job(&conn, &job.id).unwrap();
        assert_eq!(job.status, ConversionStatus::Cancelled);
    }

    #[test]
    fn test_dequeue_next_job() {
        let conn = setup_test_db();
        let (item_id, file_id) = create_test_item_and_file(&conn);

        // No jobs initially
        assert!(dequeue_next_job(&conn).unwrap().is_none());

        // Create a job
        create_conversion_job(&conn, item_id, file_id).unwrap();

        // Should get the job
        let job = dequeue_next_job(&conn).unwrap();
        assert!(job.is_some());
    }

    #[test]
    fn test_get_active_job_for_item() {
        let conn = setup_test_db();
        let (item_id, file_id) = create_test_item_and_file(&conn);

        // No active job initially
        assert!(get_active_job_for_item(&conn, item_id).unwrap().is_none());

        // Create and start a job
        let job = create_conversion_job(&conn, item_id, file_id).unwrap();
        start_job(&conn, &job.id, None).unwrap();

        // Should find the active job
        let active = get_active_job_for_item(&conn, item_id).unwrap();
        assert!(active.is_some());
        assert_eq!(active.unwrap().id, job.id);

        // Complete the job
        complete_job(&conn, &job.id, "/output.mp4").unwrap();

        // No active job now
        assert!(get_active_job_for_item(&conn, item_id).unwrap().is_none());
    }
}
