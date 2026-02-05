//! Profile B (H.264/AAC) encoding using ffmpeg.

use std::path::Path;
use std::time::Duration;

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

/// Convert a source media file to Profile B (H.264 High / AAC-LC stereo).
///
/// Uses adaptive CRF based on source resolution unless overridden in config.
/// Output is an MP4 file with `+faststart` for progressive download.
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

    tracing::info!(
        "Profile B encode: {:?} -> {:?} (crf={}, preset={})",
        input,
        output,
        crf,
        config.video_preset
    );

    let mut cmd = ToolCommand::new(ffmpeg.path.clone());
    cmd.timeout(Duration::from_secs(86400)); // 24 hours
    cmd.args(["-y", "-i"]);
    cmd.arg(input.to_string_lossy().as_ref());
    cmd.args(["-c:v", "libx264", "-profile:v", "high"]);
    cmd.args(["-crf", &crf.to_string()]);
    cmd.args(["-preset", &config.video_preset]);
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
}
