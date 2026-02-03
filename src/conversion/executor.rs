//! Conversion job executor.
//!
//! Processes queued conversion jobs, transcoding source files to Profile B format.

use crate::probe::MediaInfo;
use crate::state::AppEvent;
use anyhow::{Context, Result};
use sceneforged_common::{FileRole, Profile};
use sceneforged_db::{
    models::ConversionStatus,
    pool::DbPool,
    queries::{conversion_jobs, media_files},
};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info};

/// Calculate adaptive CRF based on source resolution.
///
/// Lower resolution sources get lower CRF (higher quality) because:
/// - They contain less detail to begin with
/// - Quality loss is more noticeable at lower resolutions
/// - File size impact is minimal for smaller resolutions
pub fn adaptive_crf(source_info: &MediaInfo) -> u32 {
    let video = source_info.video_tracks.first();
    match video {
        Some(v) if v.height <= 480 => 12,  // SD content - preserve everything
        Some(v) if v.height <= 720 => 14,  // 720p - near lossless
        Some(v) if v.height <= 1080 => 15, // 1080p - high quality
        _ => 18,                            // 4K+ - can afford more compression
    }
}

/// Calculate adaptive CRF based on resolution dimensions.
///
/// This is a convenience function when you only have width/height, not full MediaInfo.
pub fn adaptive_crf_from_resolution(_width: u32, height: u32) -> u32 {
    if height <= 480 {
        12 // SD content - preserve everything
    } else if height <= 720 {
        14 // 720p - near lossless
    } else if height <= 1080 {
        15 // 1080p - high quality
    } else {
        18 // 4K+ - can afford more compression
    }
}

/// Profile B transcoding settings.
#[derive(Debug, Clone)]
pub struct ProfileBSettings {
    /// Maximum width (default: 1920).
    pub max_width: u32,
    /// Maximum height (default: 1080).
    pub max_height: u32,
    /// Video codec (default: h264).
    pub video_codec: String,
    /// Base video CRF (default: 15). Used when adaptive_crf is disabled.
    pub video_crf: u32,
    /// Video preset (default: slow).
    pub video_preset: String,
    /// Audio codec (default: aac).
    pub audio_codec: String,
    /// Audio bitrate (default: 256k).
    pub audio_bitrate: String,
    /// Keyframe interval in seconds (default: 2).
    pub keyframe_interval: f64,
    /// Hardware acceleration (none, nvenc, qsv, videotoolbox).
    pub hw_accel: Option<String>,
    /// Output directory for converted files.
    pub output_dir: PathBuf,
    /// Use adaptive CRF based on source resolution (default: true).
    pub adaptive_crf: bool,
}

impl Default for ProfileBSettings {
    fn default() -> Self {
        Self {
            max_width: 1920,
            max_height: 1080,
            video_codec: "h264".to_string(),
            video_crf: 15,              // Near-lossless quality
            video_preset: "slow".to_string(), // Better compression efficiency
            audio_codec: "aac".to_string(),
            audio_bitrate: "256k".to_string(), // Higher fidelity audio
            keyframe_interval: 2.0,
            hw_accel: None,
            output_dir: PathBuf::from("/tmp/sceneforged/converted"),
            adaptive_crf: true, // Use resolution-based CRF by default
        }
    }
}

/// Conversion job executor.
pub struct ConversionExecutor {
    pool: DbPool,
    settings: ProfileBSettings,
    stop_signal: Arc<AtomicBool>,
    event_tx: Option<broadcast::Sender<AppEvent>>,
}

impl ConversionExecutor {
    /// Create a new conversion executor.
    pub fn new(pool: DbPool, settings: ProfileBSettings) -> Self {
        Self {
            pool,
            settings,
            stop_signal: Arc::new(AtomicBool::new(false)),
            event_tx: None,
        }
    }

    /// Create a new conversion executor with event broadcasting.
    pub fn with_events(
        pool: DbPool,
        settings: ProfileBSettings,
        event_tx: broadcast::Sender<AppEvent>,
    ) -> Self {
        Self {
            pool,
            settings,
            stop_signal: Arc::new(AtomicBool::new(false)),
            event_tx: Some(event_tx),
        }
    }

    /// Broadcast an event if the event sender is configured.
    fn broadcast(&self, event: AppEvent) {
        if let Some(ref tx) = self.event_tx {
            if tx.send(event).is_err() {
                debug!("No subscribers for conversion event");
            }
        }
    }

    /// Get a clone of the stop signal for external control.
    pub fn stop_signal(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.stop_signal)
    }

    /// Process queued conversion jobs until stopped.
    pub fn run(&self) -> Result<()> {
        // Ensure output directory exists
        std::fs::create_dir_all(&self.settings.output_dir)?;

        info!("Conversion executor started");

        while !self.stop_signal.load(Ordering::Relaxed) {
            let conn = self.pool.get()?;

            // Get next job
            let job = match conversion_jobs::dequeue_next_job(&conn)? {
                Some(job) => job,
                None => {
                    drop(conn);
                    // No jobs, sleep and retry
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    continue;
                }
            };

            info!("Processing conversion job: {}", job.id);

            // Get source file info
            let source_file = match media_files::get_media_file(&conn, job.source_file_id) {
                Ok(f) => f,
                Err(e) => {
                    error!("Failed to get source file: {}", e);
                    let _ = conversion_jobs::fail_job(&conn, &job.id, &e.to_string());
                    continue;
                }
            };

            // Start job
            if let Err(e) =
                conversion_jobs::start_job(&conn, &job.id, self.settings.hw_accel.as_deref())
            {
                error!("Failed to start job: {}", e);
                continue;
            }
            drop(conn);

            // Generate output path
            let source_path = Path::new(&source_file.file_path);
            let file_stem = source_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            let output_path = self
                .settings
                .output_dir
                .join(format!("{}_{}.mp4", file_stem, job.source_file_id));

            // Run transcode with source height for adaptive CRF
            let source_height = source_file.height.map(|h| h as u32);
            match self.transcode(&source_file.file_path, &output_path, &job.id, source_height) {
                Ok(()) => {
                    info!("Conversion completed: {}", job.id);

                    let conn = self.pool.get()?;

                    // Complete job
                    if let Err(e) = conversion_jobs::complete_job(
                        &conn,
                        &job.id,
                        &output_path.to_string_lossy(),
                    ) {
                        error!("Failed to complete job: {}", e);
                    }

                    // Register output as universal file
                    if let Err(e) = self.register_universal_file(&conn, job.item_id, &output_path) {
                        error!("Failed to register universal file: {}", e);
                    }

                    // Broadcast PlaybackAvailable event
                    self.broadcast(AppEvent::playback_available(job.item_id.to_string()));
                }
                Err(e) => {
                    error!("Conversion failed: {} - {}", job.id, e);
                    let conn = self.pool.get()?;
                    let _ = conversion_jobs::fail_job(&conn, &job.id, &e.to_string());
                }
            }
        }

        info!("Conversion executor stopped");
        Ok(())
    }

    /// Transcode a single file to Profile B format.
    ///
    /// # Arguments
    /// * `input` - Source file path
    /// * `output` - Output file path
    /// * `_job_id` - Job identifier (for future progress tracking)
    /// * `source_height` - Source video height for adaptive CRF calculation
    fn transcode(
        &self,
        input: &str,
        output: &Path,
        _job_id: &str,
        source_height: Option<u32>,
    ) -> Result<()> {
        let mut args = vec![
            "-i".to_string(),
            input.to_string(),
            // Video settings
            "-c:v".to_string(),
        ];

        // Use hardware encoding if available
        let video_encoder = match self.settings.hw_accel.as_deref() {
            Some("nvenc") => "h264_nvenc",
            Some("qsv") => "h264_qsv",
            Some("videotoolbox") => "h264_videotoolbox",
            _ => "libx264",
        };
        args.push(video_encoder.to_string());

        // Calculate CRF - use adaptive if enabled and we have source info
        let crf = if self.settings.adaptive_crf {
            if let Some(height) = source_height {
                let adaptive = adaptive_crf_from_resolution(0, height);
                debug!(
                    "Using adaptive CRF {} for source height {} (base CRF: {})",
                    adaptive, height, self.settings.video_crf
                );
                adaptive
            } else {
                self.settings.video_crf
            }
        } else {
            self.settings.video_crf
        };

        // Video quality settings
        if video_encoder == "libx264" {
            args.extend([
                "-crf".to_string(),
                crf.to_string(),
                "-preset".to_string(),
                self.settings.video_preset.clone(),
            ]);
        } else {
            // Hardware encoders use different quality settings
            args.extend([
                "-b:v".to_string(),
                "5M".to_string(), // 5 Mbps target for 1080p
                "-maxrate".to_string(),
                "8M".to_string(),
                "-bufsize".to_string(),
                "16M".to_string(),
            ]);
        }

        // Video profile
        args.extend(["-profile:v".to_string(), "high".to_string()]);

        // Scale down if needed (preserving aspect ratio)
        args.extend([
            "-vf".to_string(),
            format!(
                "scale='min({},iw)':'min({},ih)':force_original_aspect_ratio=decrease",
                self.settings.max_width, self.settings.max_height
            ),
        ]);

        // Keyframe interval for HLS
        let keyframe_expr = format!("expr:gte(t,n_forced*{})", self.settings.keyframe_interval);
        args.extend(["-force_key_frames".to_string(), keyframe_expr]);

        // Audio settings
        args.extend([
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            self.settings.audio_bitrate.clone(),
            "-ac".to_string(),
            "2".to_string(), // Stereo
        ]);

        // MP4 faststart for streaming
        args.extend(["-movflags".to_string(), "+faststart".to_string()]);

        // Output
        args.extend([
            "-y".to_string(), // Overwrite
            output.to_string_lossy().to_string(),
        ]);

        debug!("FFmpeg args: {:?}", args);

        let status = Command::new("ffmpeg")
            .args(&args)
            .status()
            .context("Failed to execute ffmpeg")?;

        if !status.success() {
            anyhow::bail!("FFmpeg exited with status: {}", status);
        }

        Ok(())
    }

    /// Register the converted file as a universal media file.
    fn register_universal_file(
        &self,
        conn: &rusqlite::Connection,
        item_id: sceneforged_common::ItemId,
        output_path: &Path,
    ) -> Result<()> {
        let file_size = std::fs::metadata(output_path)?.len() as i64;

        // Create media file entry with Profile::B (universal playback profile)
        let media_file = media_files::create_media_file_with_profile(
            conn,
            item_id,
            FileRole::Universal,
            Profile::B,
            &output_path.to_string_lossy(),
            file_size,
            "mp4",
        )?;

        // Update metadata
        media_files::update_media_file_metadata(
            conn,
            media_file.id,
            Some("h264"),
            Some("aac"),
            Some(self.settings.max_width as i32),
            Some(self.settings.max_height as i32),
            None,  // Duration will be read from source
            None,  // Bit rate
            false, // Not HDR
            true,  // Serves as universal
            true,  // Has faststart
            Some(self.settings.keyframe_interval),
        )?;

        info!(
            "Registered universal file: {} (Profile B) for item {}",
            media_file.id, item_id
        );

        Ok(())
    }

    /// Process a single job (useful for testing).
    pub fn process_single_job(&self, job_id: &str) -> Result<()> {
        let conn = self.pool.get()?;

        let job = conversion_jobs::get_conversion_job(&conn, job_id)?;

        if job.status != ConversionStatus::Queued {
            anyhow::bail!("Job {} is not in queued state", job_id);
        }

        // Get source file
        let source_file = media_files::get_media_file(&conn, job.source_file_id)?;

        // Start job
        conversion_jobs::start_job(&conn, job_id, self.settings.hw_accel.as_deref())?;

        // Generate output path
        let source_path = Path::new(&source_file.file_path);
        let file_stem = source_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let output_path = self
            .settings
            .output_dir
            .join(format!("{}_{}.mp4", file_stem, job.source_file_id));

        // Run transcode with source height for adaptive CRF
        let source_height = source_file.height.map(|h| h as u32);
        match self.transcode(&source_file.file_path, &output_path, job_id, source_height) {
            Ok(()) => {
                conversion_jobs::complete_job(&conn, job_id, &output_path.to_string_lossy())?;
                self.register_universal_file(&conn, job.item_id, &output_path)?;
                Ok(())
            }
            Err(e) => {
                let _ = conversion_jobs::fail_job(&conn, job_id, &e.to_string());
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = ProfileBSettings::default();
        assert_eq!(settings.max_width, 1920);
        assert_eq!(settings.max_height, 1080);
        assert_eq!(settings.video_codec, "h264");
        assert_eq!(settings.video_crf, 15); // Near-lossless
        assert_eq!(settings.video_preset, "slow");
        assert_eq!(settings.audio_bitrate, "256k");
        assert_eq!(settings.keyframe_interval, 2.0);
        assert!(settings.adaptive_crf);
    }

    #[test]
    fn test_adaptive_crf_sd() {
        // 480p SD content gets CRF 12 (preserve everything)
        assert_eq!(adaptive_crf_from_resolution(640, 480), 12);
        assert_eq!(adaptive_crf_from_resolution(720, 480), 12);
        // Even smaller
        assert_eq!(adaptive_crf_from_resolution(320, 240), 12);
    }

    #[test]
    fn test_adaptive_crf_720p() {
        // 720p gets CRF 14
        assert_eq!(adaptive_crf_from_resolution(1280, 720), 14);
        // Just above SD
        assert_eq!(adaptive_crf_from_resolution(960, 540), 14);
    }

    #[test]
    fn test_adaptive_crf_1080p() {
        // 1080p gets CRF 15
        assert_eq!(adaptive_crf_from_resolution(1920, 1080), 15);
        // Just above 720p
        assert_eq!(adaptive_crf_from_resolution(1280, 800), 15);
    }

    #[test]
    fn test_adaptive_crf_4k() {
        // 4K and above gets CRF 18 (more compression is acceptable)
        assert_eq!(adaptive_crf_from_resolution(3840, 2160), 18);
        assert_eq!(adaptive_crf_from_resolution(4096, 2160), 18);
        // Just above 1080p
        assert_eq!(adaptive_crf_from_resolution(2560, 1440), 18);
    }
}
