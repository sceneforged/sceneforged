//! Background job processor.
//!
//! Polls the database for queued jobs, probes each file, matches against rules,
//! creates and executes a pipeline, and updates job status throughout.
//! Retries with exponential backoff on failure.

use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use sf_core::events::{EventCategory, EventPayload};
use sf_pipeline::{create_actions, ActionContext, PipelineExecutor, ProgressSender};
use sf_rules::RuleEngine;

use crate::context::AppContext;
use crate::notifications::{self, NotificationManager};

/// Worker identifier for locking jobs.
const WORKER_ID: &str = "sf-processor";

/// Start the background job processor.
///
/// Runs until the cancellation token is triggered.
pub async fn run_processor(ctx: AppContext, cancel: CancellationToken) {
    tracing::info!("Job processor started");

    loop {
        if cancel.is_cancelled() {
            tracing::info!("Job processor shutting down");
            break;
        }

        match process_next_job(&ctx).await {
            Ok(true) => {
                // Processed a job; immediately check for the next one.
                continue;
            }
            Ok(false) => {
                // No jobs available; wait before polling again.
            }
            Err(e) => {
                tracing::error!("Job processor error: {e}");
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(2)) => {}
            _ = cancel.cancelled() => { break; }
        }
    }

    tracing::info!("Job processor stopped");
}

/// Try to dequeue and process the next job.
///
/// Returns `Ok(true)` if a job was processed, `Ok(false)` if no jobs were
/// available.
async fn process_next_job(ctx: &AppContext) -> sf_core::Result<bool> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let job = sf_db::queries::jobs::dequeue_next(&conn, WORKER_ID)?;
    drop(conn);

    let Some(job) = job else {
        return Ok(false);
    };

    let job_id = job.id;
    tracing::info!(job_id = %job_id, file = %job.file_path, "Processing job");

    ctx.event_bus.broadcast(
        EventCategory::Admin,
        EventPayload::JobStarted { job_id },
    );

    match execute_job(ctx, &job).await {
        Ok(()) => {
            let conn = sf_db::pool::get_conn(&ctx.db)?;
            sf_db::queries::jobs::complete_job(&conn, job_id)?;
            ctx.event_bus.broadcast(
                EventCategory::Admin,
                EventPayload::JobCompleted { job_id },
            );
            tracing::info!(job_id = %job_id, "Job completed");

            // Fire post-completion notifications (non-blocking).
            fire_post_job_notifications(ctx, &job);
        }
        Err(e) => {
            let error_msg = e.to_string();
            tracing::error!(job_id = %job_id, error = %error_msg, "Job failed");

            let conn = sf_db::pool::get_conn(&ctx.db)?;
            sf_db::queries::jobs::fail_job(&conn, job_id, &error_msg)?;

            // Auto-retry with exponential backoff if retries remain.
            if job.retry_count < job.max_retries {
                let backoff = Duration::from_secs(2u64.pow(job.retry_count as u32).min(300));
                tracing::info!(
                    job_id = %job_id,
                    retry = job.retry_count + 1,
                    backoff_secs = backoff.as_secs(),
                    "Scheduling retry"
                );
                tokio::time::sleep(backoff).await;
                sf_db::queries::jobs::retry_job(&conn, job_id)?;
            }

            ctx.event_bus.broadcast(
                EventCategory::Admin,
                EventPayload::JobFailed {
                    job_id,
                    error: error_msg,
                },
            );
        }
    }

    Ok(true)
}

/// Execute the pipeline for a single job.
async fn execute_job(ctx: &AppContext, job: &sf_db::models::Job) -> sf_core::Result<()> {
    let path = std::path::Path::new(&job.file_path);

    // Probe the file.
    let media_info = ctx.prober.probe(path)?;

    // Match rules.
    let rules = ctx.config_store.get_rules();
    let engine = RuleEngine::new(rules);
    let matched_rule = engine
        .find_matching_rule(&media_info)
        .ok_or_else(|| sf_core::Error::Validation("No matching rule found".into()))?;

    // Update the job with the matched rule name.
    {
        let conn = sf_db::pool::get_conn(&ctx.db)?;
        sf_db::queries::jobs::update_job_progress(&conn, job.id, 0.0, Some(&matched_rule.name))?;
    }

    // Create actions from the rule.
    let actions = create_actions(&matched_rule.actions, &ctx.tools)?;

    if actions.is_empty() {
        return Ok(());
    }

    // Set up workspace and context.
    let workspace = Arc::new(sf_av::Workspace::new(path)?);
    let media_info = Arc::new(media_info);

    let job_id = job.id;
    let db = ctx.db.clone();
    let event_bus = ctx.event_bus.clone();

    let progress = ProgressSender::new(move |pct, step| {
        let _ = (|| -> sf_core::Result<()> {
            let conn = sf_db::pool::get_conn(&db)?;
            sf_db::queries::jobs::update_job_progress(
                &conn,
                job_id,
                (pct / 100.0) as f64,
                Some(step),
            )?;
            Ok(())
        })();

        event_bus.broadcast(
            EventCategory::Admin,
            EventPayload::JobProgress {
                job_id,
                progress: pct / 100.0,
                step: step.to_string(),
            },
        );
    });

    let action_ctx = ActionContext::new(workspace, media_info, ctx.tools.clone())
        .with_progress(progress);

    // Execute the pipeline.
    let executor = PipelineExecutor::new(actions);
    executor.execute(&action_ctx).await?;

    Ok(())
}

/// Fire non-blocking notifications to Jellyfin and *arr services after a job
/// completes successfully. Errors in notifications are logged but never fail
/// the job.
fn fire_post_job_notifications(ctx: &AppContext, job: &sf_db::models::Job) {
    let manager = NotificationManager::new();

    // Notify all enabled Jellyfin instances.
    let jellyfins = ctx.config_store.jellyfins.read().clone();
    notifications::spawn_jellyfin_notifications(&manager, jellyfins);

    // Notify all enabled arr instances with auto_rescan.
    let arrs = ctx.config_store.arrs.read().clone();
    notifications::spawn_arr_notifications(&manager, arrs, job.file_path.clone());
}
