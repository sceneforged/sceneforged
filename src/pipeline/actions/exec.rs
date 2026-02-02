use crate::pipeline::{TemplateContext, Workspace};
use anyhow::{Context, Result};
use std::process::Command;

/// Execute a custom command with template variable substitution
pub fn exec_command(workspace: &Workspace, command: &str, args: &[String]) -> Result<()> {
    let ctx = TemplateContext::new().with_workspace(
        workspace.input(),
        workspace.output(),
        workspace.temp_dir(),
    );

    let expanded_command = ctx.substitute(command);
    let expanded_args: Vec<String> = args.iter().map(|a| ctx.substitute(a)).collect();

    tracing::info!(
        "Executing custom command: {} {:?}",
        expanded_command,
        expanded_args
    );

    let result = Command::new(&expanded_command)
        .args(&expanded_args)
        .output()
        .with_context(|| format!("Failed to execute: {}", expanded_command))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        let stdout = String::from_utf8_lossy(&result.stdout);
        anyhow::bail!(
            "Command failed with exit code {:?}\nStdout: {}\nStderr: {}",
            result.status.code(),
            stdout,
            stderr
        );
    }

    tracing::debug!(
        "Command output: {}",
        String::from_utf8_lossy(&result.stdout)
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    // Integration tests would require command execution
    // Unit tests for template substitution are in template.rs
}
