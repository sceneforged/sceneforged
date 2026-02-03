//! Dolby Vision profile conversion via `dovi_tool`.
//!
//! The process:
//! 1. Extract HEVC elementary stream (ffmpeg)
//! 2. Convert DV profile in-place (dovi_tool convert)
//! 3. Remux back with original audio and subtitles (mkvmerge)

use crate::command::ToolCommand;
use crate::tools::ToolRegistry;
use crate::workspace::Workspace;

/// Convert the Dolby Vision profile of the workspace input file.
///
/// `target_profile` is typically `8` (convert profile 7 to profile 8.1).
pub async fn convert_dv_profile(
    workspace: &Workspace,
    tools: &ToolRegistry,
    target_profile: u8,
) -> sf_core::Result<()> {
    let input = workspace.input();
    let output = workspace.output();

    tracing::info!(
        "convert DV to profile {target_profile} for {:?}",
        input
    );

    let ffmpeg = tools.require("ffmpeg")?;
    let dovi_tool = tools.require("dovi_tool")?;
    let mkvmerge = tools.require("mkvmerge")?;

    // Step 1: Extract HEVC elementary stream.
    let hevc_file = workspace.temp_file("video.hevc");
    {
        let mut cmd = ToolCommand::new(ffmpeg.path.clone());
        cmd.args(["-y", "-i"]);
        cmd.arg(input.to_string_lossy().as_ref());
        cmd.args([
            "-c:v", "copy",
            "-bsf:v", "hevc_mp4toannexb",
            "-an", "-sn",
            "-f", "hevc",
        ]);
        cmd.arg(hevc_file.to_string_lossy().as_ref());
        cmd.execute().await?;
    }

    // Step 2: Run dovi_tool convert on the HEVC file.
    let converted_hevc = workspace.temp_file("video_converted.hevc");
    {
        let mode = match target_profile {
            8 => "2",   // mode 2 = convert to P8.1
            _ => "2",   // default to P8.1
        };

        let mut cmd = ToolCommand::new(dovi_tool.path.clone());
        cmd.args(["convert", "--mode", mode, "-i"]);
        cmd.arg(hevc_file.to_string_lossy().as_ref());
        cmd.arg("-o");
        cmd.arg(converted_hevc.to_string_lossy().as_ref());
        cmd.execute().await?;
    }

    // Step 3: Remux converted video with original audio and subtitles.
    {
        let mut cmd = ToolCommand::new(mkvmerge.path.clone());
        cmd.arg("-o");
        cmd.arg(output.to_string_lossy().as_ref());
        // New video track.
        cmd.arg(converted_hevc.to_string_lossy().as_ref());
        // Audio and subtitles from original (no video).
        cmd.arg("--no-video");
        cmd.arg(input.to_string_lossy().as_ref());
        cmd.execute().await?;
    }

    tracing::info!("DV conversion complete: {:?}", output);
    Ok(())
}
