//! Background conversion processor.
//!
//! Polls the database for queued conversion jobs, encodes to Profile B,
//! generates HLS segments, and updates job status throughout.

use std::path::Path;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use sf_core::events::{EventCategory, EventPayload};

use crate::context::AppContext;
use crate::notifications::{self, NotificationManager};

/// Worker identifier for locking conversion jobs.
const WORKER_ID: &str = "sf-conversion";

/// Start the background conversion processor.
///
/// Runs until the cancellation token is triggered.
pub async fn run_conversion_processor(ctx: AppContext, cancel: CancellationToken) {
    tracing::info!("Conversion processor started");

    loop {
        if cancel.is_cancelled() {
            tracing::info!("Conversion processor shutting down");
            break;
        }

        match process_next_conversion(&ctx).await {
            Ok(true) => {
                // Processed a job; immediately check for the next one.
                continue;
            }
            Ok(false) => {
                // No jobs available; wait before polling again.
            }
            Err(e) => {
                tracing::error!("Conversion processor error: {e}");
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(2)) => {}
            _ = cancel.cancelled() => { break; }
        }
    }

    tracing::info!("Conversion processor stopped");
}

/// Try to dequeue and process the next conversion job.
///
/// Returns `Ok(true)` if a job was processed, `Ok(false)` if no jobs were available.
async fn process_next_conversion(ctx: &AppContext) -> sf_core::Result<bool> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let job = sf_db::queries::conversion_jobs::dequeue_next_conversion(&conn, WORKER_ID)?;
    drop(conn);

    let Some(job) = job else {
        return Ok(false);
    };

    let job_id = job.id;
    tracing::info!(job_id = %job_id, item_id = %job.item_id, "Processing conversion job");

    ctx.event_bus.broadcast(
        EventCategory::Admin,
        EventPayload::ConversionStarted { job_id },
    );

    match execute_conversion(ctx, &job).await {
        Ok(()) => {
            tracing::info!(job_id = %job_id, "Conversion completed");

            // Fire post-completion notifications (non-blocking).
            fire_post_conversion_notifications(ctx, &job);
        }
        Err(e) => {
            let error_msg = e.to_string();
            tracing::error!(job_id = %job_id, error = %error_msg, "Conversion failed");

            let conn = sf_db::pool::get_conn(&ctx.db)?;
            sf_db::queries::conversion_jobs::fail_conversion(&conn, job_id, &error_msg)?;

            ctx.event_bus.broadcast(
                EventCategory::Admin,
                EventPayload::ConversionFailed {
                    job_id,
                    error: error_msg,
                },
            );
        }
    }

    Ok(true)
}

/// Execute the conversion pipeline for a single job.
async fn execute_conversion(
    ctx: &AppContext,
    job: &sf_db::models::ConversionJob,
) -> sf_core::Result<()> {
    let job_id = job.id;
    let item_id = job.item_id;

    // Look up the source media file.
    let source_mf_id = job
        .source_media_file_id
        .ok_or_else(|| sf_core::Error::Validation("No source_media_file_id on job".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let source_mf = sf_db::queries::media_files::get_media_file(&conn, source_mf_id)?
        .ok_or_else(|| sf_core::Error::not_found("media_file", source_mf_id))?;
    drop(conn);

    let source_path = Path::new(&source_mf.file_path);
    let source_height = source_mf.resolution_height.map(|h| h as u32);

    // Compute output path: <source_dir>/<source_stem>-pb.mp4
    let source_dir = source_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let source_stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let output_path = source_dir.join(format!("{source_stem}-pb.mp4"));

    // Get conversion config.
    let config = ctx.config_store.conversion.read().clone();

    // Run Profile B encoding.
    sf_av::convert_to_profile_b(
        &ctx.tools,
        source_path,
        &output_path,
        source_height,
        &config,
    )
    .await?;

    // Update progress to 90% (encoding done, HLS segmenting next).
    {
        let conn = sf_db::pool::get_conn(&ctx.db)?;
        sf_db::queries::conversion_jobs::update_conversion_progress(
            &conn,
            job_id,
            90.0,
            None,
            None,
        )?;
    }

    ctx.event_bus.broadcast(
        EventCategory::Admin,
        EventPayload::ConversionProgress {
            job_id,
            progress: 0.9,
        },
    );

    // Register the output as a media file.
    let output_file_size = std::fs::metadata(&output_path)
        .map(|m| m.len() as i64)
        .unwrap_or(0);
    let output_file_name = output_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("output-pb.mp4")
        .to_string();

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let output_mf = sf_db::queries::media_files::create_media_file(
        &conn,
        item_id,
        &output_path.to_string_lossy(),
        &output_file_name,
        output_file_size,
        Some("mp4"),
        Some("h264"),
        Some("aac"),
        source_mf.resolution_width,  // Preserved (may be scaled down, but close enough)
        source_mf.resolution_height,
        None,   // No HDR in Profile B
        false,  // No Dolby Vision
        None,
        "universal",
        "B",
        source_mf.duration_secs,
    )?;

    // Generate HLS segments.
    let hls_dir = output_path.with_extension("").with_extension("hls");
    sf_av::generate_hls_segments(&ctx.tools, &output_path, &hls_dir, 6).await?;

    // Upsert HLS cache.
    sf_db::queries::hls_cache::upsert_hls_cache(
        &conn,
        output_mf.id,
        &hls_dir.to_string_lossy(),
    )?;

    // Complete the conversion job.
    sf_db::queries::conversion_jobs::complete_conversion(&conn, job_id, output_mf.id)?;

    ctx.event_bus.broadcast(
        EventCategory::Admin,
        EventPayload::ConversionCompleted { job_id },
    );

    Ok(())
}

/// Fire non-blocking notifications to Jellyfin and *arr services after a
/// conversion completes successfully. Errors in notifications are logged but
/// never fail the conversion.
fn fire_post_conversion_notifications(
    ctx: &AppContext,
    job: &sf_db::models::ConversionJob,
) {
    let manager = NotificationManager::new();

    // Notify all enabled Jellyfin instances.
    let jellyfins = ctx.config_store.jellyfins.read().clone();
    notifications::spawn_jellyfin_notifications(&manager, jellyfins);

    // Notify all enabled arr instances with auto_rescan.
    // For conversion jobs we don't have a direct file_path on the job model,
    // so we look up the source media file path from the database.
    let file_path = job
        .source_media_file_id
        .and_then(|mf_id| {
            sf_db::pool::get_conn(&ctx.db)
                .ok()
                .and_then(|conn| {
                    sf_db::queries::media_files::get_media_file(&conn, mf_id)
                        .ok()
                        .flatten()
                        .map(|mf| mf.file_path)
                })
        })
        .unwrap_or_default();

    let arrs = ctx.config_store.arrs.read().clone();
    notifications::spawn_arr_notifications(&manager, arrs, file_path);
}
