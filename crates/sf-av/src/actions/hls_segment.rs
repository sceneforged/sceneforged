//! HLS fMP4 segment generation using ffmpeg `-c copy`.

use std::path::Path;
use std::time::Duration;

use crate::command::ToolCommand;
use crate::tools::ToolRegistry;

/// Generate HLS fMP4 segments from an already-encoded MP4 file.
///
/// This uses `-c copy` so it is very fast (no re-encoding). Produces:
/// - `<output_dir>/index.m3u8` — HLS playlist
/// - `<output_dir>/init.mp4` — fMP4 initialization segment
/// - `<output_dir>/seg0000.m4s`, `seg0001.m4s`, … — media segments
pub async fn generate_hls_segments(
    tools: &ToolRegistry,
    input_mp4: &Path,
    output_dir: &Path,
    target_segment_duration: u32,
) -> sf_core::Result<()> {
    let ffmpeg = tools.require("ffmpeg")?;

    // Ensure output directory exists.
    std::fs::create_dir_all(output_dir).map_err(|e| {
        sf_core::Error::Internal(format!(
            "Failed to create HLS output dir {}: {e}",
            output_dir.display()
        ))
    })?;

    let seg_pattern = output_dir.join("seg%04d.m4s");
    let init_filename = "init.mp4";
    let playlist_path = output_dir.join("index.m3u8");

    tracing::info!(
        "HLS segment: {:?} -> {:?} (segment_duration={}s)",
        input_mp4,
        output_dir,
        target_segment_duration
    );

    let mut cmd = ToolCommand::new(ffmpeg.path.clone());
    cmd.timeout(Duration::from_secs(600)); // 10 minutes (copy is fast)
    cmd.args(["-y", "-i"]);
    cmd.arg(input_mp4.to_string_lossy().as_ref());
    cmd.args(["-c", "copy", "-f", "hls"]);
    cmd.args(["-hls_time", &target_segment_duration.to_string()]);
    cmd.args(["-hls_segment_type", "fmp4"]);
    cmd.args(["-hls_playlist_type", "vod"]);
    cmd.args(["-hls_segment_filename", &seg_pattern.to_string_lossy()]);
    cmd.args(["-hls_fmp4_init_filename", init_filename]);
    cmd.arg(playlist_path.to_string_lossy().as_ref());
    cmd.execute().await?;

    Ok(())
}
