//! Job queue operations.

use chrono::Utc;
use rusqlite::Connection;
use sf_core::{Error, JobId, Result};

use crate::models::Job;

const COLS: &str = "id, file_path, file_name, status, rule_name, progress,
    current_step, error, source, retry_count, max_retries, priority,
    locked_by, locked_at, created_at, started_at, completed_at, scheduled_for";

/// Create a new job.
pub fn create_job(
    conn: &Connection,
    file_path: &str,
    file_name: &str,
    source: Option<&str>,
    priority: i32,
) -> Result<Job> {
    let id = JobId::new();
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO jobs (id, file_path, file_name, status, source, priority, created_at)
         VALUES (?1, ?2, ?3, 'queued', ?4, ?5, ?6)",
        rusqlite::params![id.to_string(), file_path, file_name, source, priority, &now],
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(Job {
        id,
        file_path: file_path.to_string(),
        file_name: file_name.to_string(),
        status: "queued".to_string(),
        rule_name: None,
        progress: 0.0,
        current_step: None,
        error: None,
        source: source.map(String::from),
        retry_count: 0,
        max_retries: 3,
        priority,
        locked_by: None,
        locked_at: None,
        created_at: now,
        started_at: None,
        completed_at: None,
        scheduled_for: None,
    })
}

/// Get a job by ID.
pub fn get_job(conn: &Connection, id: JobId) -> Result<Option<Job>> {
    let q = format!("SELECT {COLS} FROM jobs WHERE id = ?1");
    let result = conn.query_row(&q, [id.to_string()], Job::from_row);
    match result {
        Ok(j) => Ok(Some(j)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// List jobs with optional status filter and pagination.
pub fn list_jobs(
    conn: &Connection,
    status: Option<&str>,
    offset: i64,
    limit: i64,
) -> Result<Vec<Job>> {
    let (q, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(s) = status {
        (
            format!(
                "SELECT {COLS} FROM jobs WHERE status = ?1
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
                "SELECT {COLS} FROM jobs
                 ORDER BY priority DESC, created_at ASC LIMIT ?1 OFFSET ?2"
            ),
            vec![Box::new(limit), Box::new(offset)],
        )
    };

    let mut stmt = conn.prepare(&q).map_err(|e| Error::database(e.to_string()))?;
    let params_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();
    let rows = stmt
        .query_map(params_refs.as_slice(), Job::from_row)
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(rows)
}

/// Update a job's status.
pub fn update_job_status(conn: &Connection, id: JobId, status: &str) -> Result<bool> {
    let n = conn
        .execute(
            "UPDATE jobs SET status = ?1 WHERE id = ?2",
            rusqlite::params![status, id.to_string()],
        )
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

/// Update a job's progress and optional current_step.
pub fn update_job_progress(
    conn: &Connection,
    id: JobId,
    progress: f64,
    current_step: Option<&str>,
) -> Result<bool> {
    let n = conn
        .execute(
            "UPDATE jobs SET progress = ?1, current_step = ?2 WHERE id = ?3",
            rusqlite::params![progress, current_step, id.to_string()],
        )
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

/// Atomically dequeue the next queued job.
///
/// Sets `status='processing'`, `locked_by`, `locked_at`, `started_at`.
/// Uses a sub-select to pick the highest-priority, oldest job.
pub fn dequeue_next(conn: &Connection, worker: &str) -> Result<Option<Job>> {
    let now = Utc::now().to_rfc3339();

    // SQLite RETURNING is supported since 3.35.
    let q = format!(
        "UPDATE jobs SET status='processing', locked_by=?1, locked_at=?2, started_at=?2
         WHERE id = (
             SELECT id FROM jobs WHERE status='queued'
             ORDER BY priority DESC, created_at ASC LIMIT 1
         )
         RETURNING {COLS}"
    );

    let result = conn.query_row(&q, rusqlite::params![worker, &now], Job::from_row);
    match result {
        Ok(j) => Ok(Some(j)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Mark a job as failed.
pub fn fail_job(conn: &Connection, id: JobId, error: &str) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let n = conn
        .execute(
            "UPDATE jobs SET status='failed', error=?1, completed_at=?2 WHERE id=?3",
            rusqlite::params![error, now, id.to_string()],
        )
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

/// Mark a job as completed.
pub fn complete_job(conn: &Connection, id: JobId) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let n = conn
        .execute(
            "UPDATE jobs SET status='completed', progress=1.0, completed_at=?1 WHERE id=?2",
            rusqlite::params![now, id.to_string()],
        )
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

/// Re-queue a failed job for retry (increments retry_count, resets status).
pub fn retry_job(conn: &Connection, id: JobId) -> Result<bool> {
    let n = conn
        .execute(
            "UPDATE jobs SET status='queued', error=NULL, locked_by=NULL, locked_at=NULL,
                started_at=NULL, completed_at=NULL, retry_count=retry_count+1
             WHERE id=?1 AND retry_count < max_retries",
            [id.to_string()],
        )
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_memory_pool;

    #[test]
    fn create_and_get() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let job = create_job(&conn, "/file.mkv", "file.mkv", Some("scan"), 0).unwrap();
        assert_eq!(job.status, "queued");

        let found = get_job(&conn, job.id).unwrap().unwrap();
        assert_eq!(found.file_path, "/file.mkv");
    }

    #[test]
    fn list_with_filter() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        create_job(&conn, "/a.mkv", "a.mkv", None, 0).unwrap();
        create_job(&conn, "/b.mkv", "b.mkv", None, 0).unwrap();

        let all = list_jobs(&conn, None, 0, 100).unwrap();
        assert_eq!(all.len(), 2);

        let queued = list_jobs(&conn, Some("queued"), 0, 100).unwrap();
        assert_eq!(queued.len(), 2);

        let processing = list_jobs(&conn, Some("processing"), 0, 100).unwrap();
        assert!(processing.is_empty());
    }

    #[test]
    fn dequeue_and_complete() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let j1 = create_job(&conn, "/lo.mkv", "lo.mkv", None, 0).unwrap();
        let _j2 = create_job(&conn, "/hi.mkv", "hi.mkv", None, 10).unwrap();

        // should dequeue the higher-priority job first
        let dequeued = dequeue_next(&conn, "w1").unwrap().unwrap();
        assert_eq!(dequeued.file_path, "/hi.mkv");
        assert_eq!(dequeued.status, "processing");

        // complete it
        assert!(complete_job(&conn, dequeued.id).unwrap());
        let done = get_job(&conn, dequeued.id).unwrap().unwrap();
        assert_eq!(done.status, "completed");

        // dequeue next should get the low-priority one
        let next = dequeue_next(&conn, "w1").unwrap().unwrap();
        assert_eq!(next.id, j1.id);
    }

    #[test]
    fn fail_and_retry() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let job = create_job(&conn, "/f.mkv", "f.mkv", None, 0).unwrap();
        dequeue_next(&conn, "w1").unwrap();
        assert!(fail_job(&conn, job.id, "oops").unwrap());

        let failed = get_job(&conn, job.id).unwrap().unwrap();
        assert_eq!(failed.status, "failed");
        assert_eq!(failed.error.as_deref(), Some("oops"));

        assert!(retry_job(&conn, job.id).unwrap());
        let retried = get_job(&conn, job.id).unwrap().unwrap();
        assert_eq!(retried.status, "queued");
        assert_eq!(retried.retry_count, 1);
    }

    #[test]
    fn progress_update() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let job = create_job(&conn, "/p.mkv", "p.mkv", None, 0).unwrap();
        assert!(update_job_progress(&conn, job.id, 0.5, Some("remux")).unwrap());
        let found = get_job(&conn, job.id).unwrap().unwrap();
        assert!((found.progress - 0.5).abs() < f64::EPSILON);
        assert_eq!(found.current_step.as_deref(), Some("remux"));
    }
}
