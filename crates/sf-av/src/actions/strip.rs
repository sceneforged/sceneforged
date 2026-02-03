//! Strip (remove) tracks from a media file using mkvmerge.

use crate::command::ToolCommand;
use crate::tools::ToolRegistry;
use crate::workspace::Workspace;

/// Strip the specified track indices from the workspace input.
///
/// `track_indices` contains the **mkvmerge track IDs** to remove.  The
/// remaining tracks are written to the workspace output.
pub async fn strip_tracks(
    workspace: &Workspace,
    tools: &ToolRegistry,
    track_indices: &[usize],
) -> sf_core::Result<()> {
    if track_indices.is_empty() {
        // Nothing to strip -- just copy input to output.
        std::fs::copy(workspace.input(), workspace.output()).map_err(|e| {
            sf_core::Error::Tool {
                tool: "strip_tracks".into(),
                message: format!("failed to copy input: {e}"),
            }
        })?;
        return Ok(());
    }

    let input = workspace.input();
    let output = workspace.output();
    let mkvmerge = tools.require("mkvmerge")?;

    // Build comma-separated list of track IDs to exclude.
    let exclude: String = track_indices
        .iter()
        .map(|i| format!("!{i}"))
        .collect::<Vec<_>>()
        .join(",");

    tracing::info!("strip tracks {exclude} from {:?}", input);

    let mut cmd = ToolCommand::new(mkvmerge.path.clone());
    cmd.arg("-o");
    cmd.arg(output.to_string_lossy().as_ref());
    // Exclude the given audio/subtitle tracks by ID.
    cmd.args(["--audio-tracks", &exclude]);
    cmd.args(["--subtitle-tracks", &exclude]);
    cmd.arg(input.to_string_lossy().as_ref());

    cmd.execute().await?;

    Ok(())
}
