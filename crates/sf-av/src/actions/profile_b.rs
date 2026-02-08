//! Profile B (H.264/AAC) encoding using ffmpeg.

use std::path::Path;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::command::ToolCommand;
use crate::tools::ToolRegistry;

/// Compute adaptive CRF based on source resolution height.
///
/// - SD (≤480p): CRF 12
/// - 720p (≤720p): CRF 14
/// - 1080p (≤1080p): CRF 15
/// - 4K+: CRF 18
pub fn adaptive_crf(height: u32) -> u32 {
    if height <= 480 {
        12
    } else if height <= 720 {
        14
    } else if height <= 1080 {
        15
    } else {
        18
    }
}

/// Resolve hardware acceleration method to the appropriate hwaccel flags and
/// encoder name for ffmpeg.
///
/// Returns `(hwaccel_args, encoder, use_crf)` where:
/// - `hwaccel_args` are the `-hwaccel` flags to pass *before* `-i`
/// - `encoder` is the video encoder name (e.g. `libx264`, `h264_nvenc`)
/// - `use_crf` indicates whether the encoder supports CRF-based quality control
fn resolve_hw_accel(hw_accel: Option<&str>) -> (Vec<&'static str>, &'static str, bool) {
    match hw_accel {
        Some("videotoolbox") => (
            vec!["-hwaccel", "videotoolbox"],
            "h264_videotoolbox",
            false,
        ),
        Some("nvenc") => (
            vec!["-hwaccel", "cuda"],
            "h264_nvenc",
            false,
        ),
        Some("vaapi") => (
            vec!["-hwaccel", "vaapi", "-hwaccel_output_format", "vaapi"],
            "h264_vaapi",
            false,
        ),
        Some("qsv") => (
            vec!["-hwaccel", "qsv"],
            "h264_qsv",
            false,
        ),
        _ => (vec![], "libx264", true),
    }
}

/// Convert a source media file to Profile B (H.264 High / AAC-LC stereo).
///
/// Uses adaptive CRF based on source resolution unless overridden in config.
/// Output is an MP4 file with `+faststart` for progressive download.
///
/// When `hw_accel` is set in the conversion config, the corresponding hardware
/// encoder is used instead of libx264.  Hardware encoders use bitrate-based
/// quality control since they do not support CRF.
///
/// 24-hour timeout to handle very large files.
pub async fn convert_to_profile_b(
    tools: &ToolRegistry,
    input: &Path,
    output: &Path,
    source_height: Option<u32>,
    config: &sf_core::config::ConversionConfig,
) -> sf_core::Result<()> {
    let ffmpeg = tools.require("ffmpeg")?;

    let crf = if config.adaptive_crf {
        adaptive_crf(source_height.unwrap_or(1080))
    } else {
        config.video_crf
    };

    let (hwaccel_args, encoder, use_crf) =
        resolve_hw_accel(config.hw_accel.as_deref());

    tracing::info!(
        "Profile B encode: {:?} -> {:?} (encoder={}, crf={}, preset={}, hw_accel={:?})",
        input,
        output,
        encoder,
        crf,
        config.video_preset,
        config.hw_accel,
    );

    let mut cmd = ToolCommand::new(ffmpeg.path.clone());
    cmd.timeout(Duration::from_secs(86400)); // 24 hours
    cmd.args(["-y"]);

    // Hardware acceleration flags must appear before -i.
    for arg in &hwaccel_args {
        cmd.arg(*arg);
    }

    cmd.args(["-i"]);
    cmd.arg(input.to_string_lossy().as_ref());

    // Video encoder and profile.
    cmd.args(["-c:v", encoder, "-profile:v", "high"]);

    // Quality settings: CRF for software encoders, bitrate for hardware encoders.
    if use_crf {
        cmd.args(["-crf", &crf.to_string()]);
        cmd.args(["-preset", &config.video_preset]);
    } else {
        // Hardware encoders don't support CRF; use bitrate targeting.
        cmd.args(["-b:v", "5M", "-maxrate", "8M", "-bufsize", "16M"]);
    }

    cmd.args([
        "-vf",
        "scale='min(1920,iw)':'min(1080,ih)':force_original_aspect_ratio=decrease:force_divisible_by=2",
    ]);
    cmd.args(["-force_key_frames", "expr:gte(t,n_forced*2)"]);
    cmd.args(["-c:a", "aac", "-b:a", &config.audio_bitrate, "-ac", "2"]);
    cmd.args(["-movflags", "+faststart"]);
    cmd.args(["-map", "0:v:0", "-map", "0:a:0"]);
    cmd.arg(output.to_string_lossy().as_ref());
    cmd.execute().await?;

    Ok(())
}

/// Progress stats from an ffmpeg encode.
pub struct EncodeProgress {
    /// 0.0..1.0
    pub pct: f64,
    pub fps: Option<f64>,
    pub bitrate: Option<String>,
    pub speed: Option<String>,
    pub total_size: Option<i64>,
    pub frame: Option<u64>,
}

/// Like [`convert_to_profile_b`] but streams progress via a callback and
/// supports cancellation.
///
/// `duration_secs` is the source duration used to compute percentage.
/// `progress_callback` receives an [`EncodeProgress`] periodically.
pub async fn convert_to_profile_b_with_progress(
    tools: &ToolRegistry,
    input: &Path,
    output: &Path,
    source_height: Option<u32>,
    config: &sf_core::config::ConversionConfig,
    duration_secs: Option<f64>,
    mut progress_callback: impl FnMut(EncodeProgress),
    cancel: Option<CancellationToken>,
) -> sf_core::Result<()> {
    let ffmpeg = tools.require("ffmpeg")?;

    let crf = if config.adaptive_crf {
        adaptive_crf(source_height.unwrap_or(1080))
    } else {
        config.video_crf
    };

    let (hwaccel_args, encoder, use_crf) =
        resolve_hw_accel(config.hw_accel.as_deref());

    tracing::info!(
        "Profile B encode (progress): {:?} -> {:?} (encoder={}, crf={}, preset={}, hw_accel={:?})",
        input,
        output,
        encoder,
        crf,
        config.video_preset,
        config.hw_accel,
    );

    let mut cmd = ToolCommand::new(ffmpeg.path.clone());
    cmd.timeout(Duration::from_secs(86400));
    cmd.args(["-y", "-progress", "pipe:2", "-nostats"]);

    for arg in &hwaccel_args {
        cmd.arg(*arg);
    }

    cmd.args(["-i"]);
    cmd.arg(input.to_string_lossy().as_ref());

    cmd.args(["-c:v", encoder, "-profile:v", "high"]);

    if use_crf {
        cmd.args(["-crf", &crf.to_string()]);
        cmd.args(["-preset", &config.video_preset]);
    } else {
        cmd.args(["-b:v", "5M", "-maxrate", "8M", "-bufsize", "16M"]);
    }

    cmd.args([
        "-vf",
        "scale='min(1920,iw)':'min(1080,ih)':force_original_aspect_ratio=decrease:force_divisible_by=2",
    ]);
    cmd.args(["-force_key_frames", "expr:gte(t,n_forced*2)"]);
    cmd.args(["-c:a", "aac", "-b:a", &config.audio_bitrate, "-ac", "2"]);
    cmd.args(["-movflags", "+faststart"]);
    cmd.args(["-map", "0:v:0", "-map", "0:a:0"]);
    cmd.arg(output.to_string_lossy().as_ref());

    // Parse ffmpeg -progress output.
    let mut last_out_time_us: Option<i64> = None;
    let mut last_fps: Option<f64> = None;
    let mut last_bitrate: Option<String> = None;
    let mut last_speed: Option<String> = None;
    let mut last_total_size: Option<i64> = None;
    let mut last_frame: Option<u64> = None;
    let mut last_callback = std::time::Instant::now();

    cmd.execute_with_stderr_callback(
        |line| {
            if let Some(val) = line.strip_prefix("out_time_us=") {
                last_out_time_us = val.trim().parse::<i64>().ok();
            } else if let Some(val) = line.strip_prefix("fps=") {
                last_fps = val.trim().parse::<f64>().ok();
            } else if let Some(val) = line.strip_prefix("bitrate=") {
                let v = val.trim();
                if v != "N/A" {
                    last_bitrate = Some(v.to_string());
                }
            } else if let Some(val) = line.strip_prefix("speed=") {
                let v = val.trim();
                if v != "N/A" {
                    last_speed = Some(v.to_string());
                }
            } else if let Some(val) = line.strip_prefix("total_size=") {
                last_total_size = val.trim().parse::<i64>().ok();
            } else if let Some(val) = line.strip_prefix("frame=") {
                last_frame = val.trim().parse::<u64>().ok();
            } else if line.starts_with("progress=") {
                // End of a progress block — emit callback.
                if let (Some(out_us), Some(dur)) = (last_out_time_us, duration_secs) {
                    if dur > 0.0 {
                        let elapsed_secs = out_us as f64 / 1_000_000.0;
                        let pct = (elapsed_secs / dur).clamp(0.0, 1.0);
                        // Throttle to ~2 second intervals.
                        let now = std::time::Instant::now();
                        if now.duration_since(last_callback) >= Duration::from_secs(2)
                            || line.contains("end")
                        {
                            progress_callback(EncodeProgress {
                                pct,
                                fps: last_fps,
                                bitrate: last_bitrate.clone(),
                                speed: last_speed.clone(),
                                total_size: last_total_size,
                                frame: last_frame,
                            });
                            last_callback = now;
                        }
                    }
                }
            }
        },
        cancel,
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adaptive_crf_values() {
        assert_eq!(adaptive_crf(480), 12);
        assert_eq!(adaptive_crf(360), 12);
        assert_eq!(adaptive_crf(720), 14);
        assert_eq!(adaptive_crf(1080), 15);
        assert_eq!(adaptive_crf(2160), 18);
    }

    #[test]
    fn resolve_hw_accel_none() {
        let (args, encoder, use_crf) = resolve_hw_accel(None);
        assert!(args.is_empty());
        assert_eq!(encoder, "libx264");
        assert!(use_crf);
    }

    #[test]
    fn resolve_hw_accel_explicit_none() {
        let (args, encoder, use_crf) = resolve_hw_accel(Some("none"));
        assert!(args.is_empty());
        assert_eq!(encoder, "libx264");
        assert!(use_crf);
    }

    #[test]
    fn resolve_hw_accel_videotoolbox() {
        let (args, encoder, use_crf) = resolve_hw_accel(Some("videotoolbox"));
        assert_eq!(args, vec!["-hwaccel", "videotoolbox"]);
        assert_eq!(encoder, "h264_videotoolbox");
        assert!(!use_crf);
    }

    #[test]
    fn resolve_hw_accel_nvenc() {
        let (args, encoder, use_crf) = resolve_hw_accel(Some("nvenc"));
        assert_eq!(args, vec!["-hwaccel", "cuda"]);
        assert_eq!(encoder, "h264_nvenc");
        assert!(!use_crf);
    }

    #[test]
    fn resolve_hw_accel_vaapi() {
        let (args, encoder, use_crf) = resolve_hw_accel(Some("vaapi"));
        assert_eq!(args, vec!["-hwaccel", "vaapi", "-hwaccel_output_format", "vaapi"]);
        assert_eq!(encoder, "h264_vaapi");
        assert!(!use_crf);
    }

    #[test]
    fn resolve_hw_accel_qsv() {
        let (args, encoder, use_crf) = resolve_hw_accel(Some("qsv"));
        assert_eq!(args, vec!["-hwaccel", "qsv"]);
        assert_eq!(encoder, "h264_qsv");
        assert!(!use_crf);
    }
}
