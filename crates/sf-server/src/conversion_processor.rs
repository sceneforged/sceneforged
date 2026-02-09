//! Background conversion processor.
//!
//! Polls the database for queued conversion jobs, encodes to Profile B,
//! populates the in-memory HLS cache, and updates job status throughout.

use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
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

    // Create a cancellation token for this job so it can be killed from the API.
    let job_cancel = CancellationToken::new();
    ctx.active_conversions.insert(job_id, job_cancel.clone());

    ctx.event_bus.broadcast(
        EventCategory::Admin,
        EventPayload::ConversionStarted { job_id },
    );

    let result = execute_conversion(ctx, &job, job_cancel).await;

    // Always remove from active conversions when done.
    ctx.active_conversions.remove(&job_id);

    match result {
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
    cancel: CancellationToken,
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
    let duration_secs = source_mf.duration_secs;

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

    // Use shared state for the progress callback (runs on a blocking context).
    let db = ctx.db.clone();
    let event_bus = ctx.event_bus.clone();

    // Track the last integral percent so we only write to DB on real changes.
    let last_pct = Arc::new(AtomicU8::new(0));

    // Run Profile B encoding with progress streaming.
    sf_av::convert_to_profile_b_with_progress(
        &ctx.tools,
        source_path,
        &output_path,
        source_height,
        &config,
        duration_secs,
        {
            let db = db.clone();
            let event_bus = event_bus.clone();
            let last_pct = last_pct.clone();
            move |prog: sf_av::EncodeProgress| {
                // Scale to 0..85% (encoding phase is 85% of total, HLS prep is remaining 15%).
                let scaled_pct = prog.pct * 85.0;
                let int_pct = scaled_pct as u8;

                // Only update DB/events when the integer percentage changes.
                if int_pct > last_pct.load(Ordering::Relaxed) {
                    last_pct.store(int_pct, Ordering::Relaxed);

                    if let Ok(conn) = sf_db::pool::get_conn(&db) {
                        let _ = sf_db::queries::conversion_jobs::update_conversion_progress(
                            &conn,
                            job_id,
                            scaled_pct,
                            prog.fps,
                            None,
                            prog.bitrate.as_deref(),
                            prog.speed.as_deref(),
                            prog.total_size,
                        );
                    }

                    // Estimate ETA from progress and fps.
                    let eta = if prog.pct > 0.01 {
                        duration_secs.and_then(|dur| {
                            prog.fps.map(|f| {
                                if f > 0.0 {
                                    let remaining_pct = 1.0 - prog.pct;
                                    let elapsed_pct = prog.pct;
                                    (dur * remaining_pct / elapsed_pct) as f64
                                } else {
                                    0.0
                                }
                            })
                        })
                    } else {
                        None
                    };

                    event_bus.broadcast(
                        EventCategory::Admin,
                        EventPayload::ConversionProgress {
                            job_id,
                            progress: scaled_pct as f32 / 100.0,
                            encode_fps: prog.fps,
                            eta_secs: eta,
                            bitrate: prog.bitrate,
                            speed: prog.speed,
                            total_size: prog.total_size,
                        },
                    );
                }
            }
        },
        Some(cancel),
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
            None,
            None,
            None,
        )?;
    }

    ctx.event_bus.broadcast(
        EventCategory::Admin,
        EventPayload::ConversionProgress {
            job_id,
            progress: 0.9,
            encode_fps: None,
            eta_secs: None,
            bitrate: None,
            speed: None,
            total_size: None,
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

    // Populate in-memory HLS cache from the new Profile B MP4.
    crate::hls_prep::populate_hls_cache(ctx, output_mf.id, &output_path).await?;

    // Persist the HLS blob to DB so it survives restarts.
    if let Some(entry) = ctx.hls_cache.get(&output_mf.id) {
        if let Ok(blob) = entry.0.to_bincode() {
            let db = ctx.db.clone();
            let mf_id = output_mf.id;
            // Non-fatal: if this fails, the three-tier lookup will re-parse on next restart.
            if let Ok(conn) = sf_db::pool::get_conn(&db) {
                let _ = sf_db::queries::media_files::set_hls_prepared(&conn, mf_id, &blob);
                tracing::debug!(media_file_id = %mf_id, "HLS blob persisted to DB after conversion");
            }
        }
    }

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
