//! Run an arbitrary external command in the workspace context.

use crate::command::ToolCommand;
use crate::workspace::Workspace;
use std::path::PathBuf;

/// Execute an arbitrary command, substituting `{input}` and `{output}` in
/// `args` with the workspace paths.
///
/// This is a general escape hatch for custom pipeline steps that are not
/// covered by the built-in actions.
pub async fn exec_command(
    workspace: &Workspace,
    command: &str,
    args: &[String],
) -> sf_core::Result<()> {
    let input_str = workspace.input().to_string_lossy().to_string();
    let output_str = workspace.output().to_string_lossy().to_string();

    let resolved_args: Vec<String> = args
        .iter()
        .map(|a| {
            a.replace("{input}", &input_str)
                .replace("{output}", &output_str)
        })
        .collect();

    tracing::info!("exec: {command} {}", resolved_args.join(" "));

    let mut cmd = ToolCommand::new(PathBuf::from(command));
    cmd.args(resolved_args);
    cmd.execute().await?;

    Ok(())
}
