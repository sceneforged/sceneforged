//! Container remuxing using ffmpeg or mkvmerge.

use crate::command::ToolCommand;
use crate::tools::ToolRegistry;
use crate::workspace::Workspace;

/// Remux the workspace input to a different container format.
///
/// - For MKV targets, prefers `mkvmerge` (better metadata handling), falling
///   back to `ffmpeg`.
/// - For other targets, uses `ffmpeg -c copy`.
///
/// The result is written to the workspace output path.
pub async fn remux(
    workspace: &Workspace,
    tools: &ToolRegistry,
    target_container: sf_core::Container,
) -> sf_core::Result<()> {
    let input = workspace.input();
    let output = workspace.output();
    let ext = match target_container {
        sf_core::Container::Mkv => "mkv",
        sf_core::Container::Mp4 => "mp4",
    };

    tracing::info!("remux {:?} -> {ext}", input);

    // For MKV output, try mkvmerge first.
    if target_container == sf_core::Container::Mkv {
        if let Ok(mkvmerge) = tools.require("mkvmerge") {
            let mut cmd = ToolCommand::new(mkvmerge.path.clone());
            cmd.arg("-o");
            cmd.arg(output.to_string_lossy().as_ref());
            cmd.arg(input.to_string_lossy().as_ref());

            match cmd.execute().await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    tracing::warn!("mkvmerge failed, falling back to ffmpeg: {e}");
                }
            }
        }
    }

    // Use ffmpeg.
    let ffmpeg = tools.require("ffmpeg")?;
    let mut cmd = ToolCommand::new(ffmpeg.path.clone());
    cmd.args(["-y", "-i"]);
    cmd.arg(input.to_string_lossy().as_ref());
    cmd.args(["-c", "copy"]);

    if target_container == sf_core::Container::Mp4 {
        cmd.args(["-movflags", "+faststart"]);
    }

    cmd.arg(output.to_string_lossy().as_ref());
    cmd.execute().await?;

    Ok(())
}
