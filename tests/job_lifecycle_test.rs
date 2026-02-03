//! Job lifecycle integration tests.
//!
//! Tests the job state machine transitions using the database layer directly
//! (via [`TestHarness`]) and verifying state through the API.

mod common;

use common::TestHarness;

// ---------------------------------------------------------------------------
// Queue -> dequeue -> progress -> complete
// ---------------------------------------------------------------------------

#[tokio::test]
async fn job_queue_dequeue_progress_complete() {
    let harness = TestHarness::new();
    let conn = harness.conn();

    // Create (queue) a job.
    let job =
        sf_db::queries::jobs::create_job(&conn, "/media/movie.mkv", "movie.mkv", Some("scan"), 0)
            .unwrap();
    assert_eq!(job.status, "queued");
    assert_eq!(job.retry_count, 0);

    // Dequeue the job (simulates the processor picking it up).
    let dequeued = sf_db::queries::jobs::dequeue_next(&conn, "test-worker")
        .unwrap()
        .expect("expected a job to dequeue");
    assert_eq!(dequeued.id, job.id);
    assert_eq!(dequeued.status, "processing");
    assert_eq!(dequeued.locked_by.as_deref(), Some("test-worker"));
    assert!(dequeued.started_at.is_some());

    // Update progress.
    assert!(
        sf_db::queries::jobs::update_job_progress(&conn, job.id, 0.5, Some("remuxing")).unwrap()
    );
    let updated = sf_db::queries::jobs::get_job(&conn, job.id)
        .unwrap()
        .unwrap();
    assert!((updated.progress - 0.5).abs() < f64::EPSILON);
    assert_eq!(updated.current_step.as_deref(), Some("remuxing"));

    // Complete the job.
    assert!(sf_db::queries::jobs::complete_job(&conn, job.id).unwrap());
    let completed = sf_db::queries::jobs::get_job(&conn, job.id)
        .unwrap()
        .unwrap();
    assert_eq!(completed.status, "completed");
    assert!((completed.progress - 1.0).abs() < f64::EPSILON);
    assert!(completed.completed_at.is_some());
}

// ---------------------------------------------------------------------------
// Queue -> fail -> retry -> complete
// ---------------------------------------------------------------------------

#[tokio::test]
async fn job_fail_retry_complete() {
    let harness = TestHarness::new();
    let conn = harness.conn();

    let job =
        sf_db::queries::jobs::create_job(&conn, "/media/fail.mkv", "fail.mkv", None, 0).unwrap();

    // Dequeue it.
    sf_db::queries::jobs::dequeue_next(&conn, "w1").unwrap();

    // Fail the job.
    assert!(sf_db::queries::jobs::fail_job(&conn, job.id, "disk full").unwrap());
    let failed = sf_db::queries::jobs::get_job(&conn, job.id)
        .unwrap()
        .unwrap();
    assert_eq!(failed.status, "failed");
    assert_eq!(failed.error.as_deref(), Some("disk full"));
    assert!(failed.completed_at.is_some());

    // Retry the job.
    assert!(sf_db::queries::jobs::retry_job(&conn, job.id).unwrap());
    let retried = sf_db::queries::jobs::get_job(&conn, job.id)
        .unwrap()
        .unwrap();
    assert_eq!(retried.status, "queued");
    assert_eq!(retried.retry_count, 1);
    assert!(retried.error.is_none());
    assert!(retried.locked_by.is_none());
    assert!(retried.started_at.is_none());
    assert!(retried.completed_at.is_none());

    // Dequeue again.
    let dequeued = sf_db::queries::jobs::dequeue_next(&conn, "w2")
        .unwrap()
        .unwrap();
    assert_eq!(dequeued.id, job.id);
    assert_eq!(dequeued.status, "processing");

    // Complete this time.
    assert!(sf_db::queries::jobs::complete_job(&conn, job.id).unwrap());
    let completed = sf_db::queries::jobs::get_job(&conn, job.id)
        .unwrap()
        .unwrap();
    assert_eq!(completed.status, "completed");
}

// ---------------------------------------------------------------------------
// Queue -> fail N times -> dead letter (retry refused)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn job_max_retries_becomes_dead_letter() {
    let harness = TestHarness::new();
    let conn = harness.conn();

    let job = sf_db::queries::jobs::create_job(
        &conn,
        "/media/doomed.mkv",
        "doomed.mkv",
        None,
        0,
    )
    .unwrap();

    // The default max_retries is 3.
    // The retry query condition is `retry_count < max_retries`, and each retry
    // increments retry_count. Starting from 0:
    //   fail #0: retry_count=0, retry succeeds -> retry_count=1
    //   fail #1: retry_count=1, retry succeeds -> retry_count=2
    //   fail #2: retry_count=2, retry succeeds -> retry_count=3
    //   fail #3: retry_count=3, retry REFUSED (3 < 3 is false)
    for i in 0..4 {
        sf_db::queries::jobs::dequeue_next(&conn, &format!("w{i}")).unwrap();
        sf_db::queries::jobs::fail_job(&conn, job.id, &format!("error #{i}")).unwrap();

        let retry_ok = sf_db::queries::jobs::retry_job(&conn, job.id).unwrap();
        if i < 3 {
            assert!(retry_ok, "retry #{i} should succeed");
        } else {
            // On the 4th failure retry_count == 3 == max_retries, so retry
            // should be refused.
            assert!(!retry_ok, "retry #{i} should be refused (max retries)");
        }
    }

    // Job should remain in 'failed' state -- effectively dead-lettered.
    let dead = sf_db::queries::jobs::get_job(&conn, job.id)
        .unwrap()
        .unwrap();
    assert_eq!(dead.status, "failed");
    assert_eq!(dead.retry_count, 3);
    // Further retries should still be refused.
    assert!(!sf_db::queries::jobs::retry_job(&conn, job.id).unwrap());
}

// ---------------------------------------------------------------------------
// Priority ordering in dequeue
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dequeue_respects_priority() {
    let harness = TestHarness::new();
    let conn = harness.conn();

    // Create jobs with different priorities.
    let low =
        sf_db::queries::jobs::create_job(&conn, "/lo.mkv", "lo.mkv", None, 0).unwrap();
    let _med =
        sf_db::queries::jobs::create_job(&conn, "/med.mkv", "med.mkv", None, 5).unwrap();
    let high =
        sf_db::queries::jobs::create_job(&conn, "/hi.mkv", "hi.mkv", None, 10).unwrap();

    // Dequeue should return highest priority first.
    let first = sf_db::queries::jobs::dequeue_next(&conn, "w")
        .unwrap()
        .unwrap();
    assert_eq!(first.id, high.id);

    let second = sf_db::queries::jobs::dequeue_next(&conn, "w")
        .unwrap()
        .unwrap();
    assert_eq!(second.priority, 5);

    let third = sf_db::queries::jobs::dequeue_next(&conn, "w")
        .unwrap()
        .unwrap();
    assert_eq!(third.id, low.id);

    // No more jobs to dequeue.
    assert!(sf_db::queries::jobs::dequeue_next(&conn, "w").unwrap().is_none());
}

// ---------------------------------------------------------------------------
// Job lifecycle via API
// ---------------------------------------------------------------------------

#[tokio::test]
async fn job_submit_and_delete_via_api() {
    let (_harness, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{addr}/api");

    // Submit a job.
    let resp = client
        .post(format!("{base}/jobs/submit"))
        .json(&serde_json::json!({"file_path": "/media/test.mkv"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let job: serde_json::Value = resp.json().await.unwrap();
    let job_id = job["id"].as_str().unwrap();

    // Delete (cancel) the job.
    let resp = client
        .delete(format!("{base}/jobs/{job_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // The job should now be in failed state (cancelled).
    let resp = client
        .get(format!("{base}/jobs/{job_id}"))
        .send()
        .await
        .unwrap();
    let job: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(job["status"], "failed");
}

// ---------------------------------------------------------------------------
// Job retry via API
// ---------------------------------------------------------------------------

#[tokio::test]
async fn job_retry_via_api() {
    let (harness, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();
    let base = format!("http://{addr}/api");

    // Submit a job.
    let resp = client
        .post(format!("{base}/jobs/submit"))
        .json(&serde_json::json!({"file_path": "/media/retry.mkv"}))
        .send()
        .await
        .unwrap();
    let job: serde_json::Value = resp.json().await.unwrap();
    let job_id = job["id"].as_str().unwrap().to_string();
    let job_id_parsed: sf_core::JobId = job_id.parse().unwrap();

    // Dequeue and fail the job via DB.
    {
        let conn = harness.conn();
        sf_db::queries::jobs::dequeue_next(&conn, "w").unwrap();
        sf_db::queries::jobs::fail_job(&conn, job_id_parsed, "test failure").unwrap();
    }

    // Retry the job via API.
    let resp = client
        .post(format!("{base}/jobs/{job_id}/retry"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // The job should now be queued again.
    let resp = client
        .get(format!("{base}/jobs/{job_id}"))
        .send()
        .await
        .unwrap();
    let job: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(job["status"], "queued");
}

// ---------------------------------------------------------------------------
// Empty dequeue returns None
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dequeue_from_empty_queue() {
    let harness = TestHarness::new();
    let conn = harness.conn();

    let result = sf_db::queries::jobs::dequeue_next(&conn, "w").unwrap();
    assert!(result.is_none());
}
