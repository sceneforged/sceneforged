//! External tool detection and management.

use crate::{Error, Result};
use std::path::PathBuf;
use std::process::Command;

/// Information about an external tool.
#[derive(Debug, Clone)]
pub struct ToolInfo {
    /// Name of the tool.
    pub name: String,
    /// Whether the tool is available.
    pub available: bool,
    /// Version string if available.
    pub version: Option<String>,
    /// Path to the tool executable.
    pub path: Option<PathBuf>,
}

/// Check if a tool is available and get its information.
///
/// # Example
///
/// ```no_run
/// use sceneforged_av::check_tool;
///
/// let info = check_tool("ffprobe");
/// if info.available {
///     println!("ffprobe version: {:?}", info.version);
/// }
/// ```
pub fn check_tool(name: &str) -> ToolInfo {
    check_tool_with_arg(name, "--version")
}

/// Check if a tool is available using a custom version argument.
pub fn check_tool_with_arg(name: &str, version_arg: &str) -> ToolInfo {
    let result = Command::new(name).arg(version_arg).output();

    match result {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .map(|s| s.to_string());

            let path = which::which(name).ok();

            ToolInfo {
                name: name.to_string(),
                available: true,
                version,
                path,
            }
        }
        _ => ToolInfo {
            name: name.to_string(),
            available: false,
            version: None,
            path: None,
        },
    }
}

/// Check all commonly used media tools.
///
/// Returns information about ffmpeg, ffprobe, mediainfo, mkvmerge, and dovi_tool.
pub fn check_tools() -> Vec<ToolInfo> {
    vec![
        check_tool_with_arg("ffmpeg", "-version"),
        check_tool_with_arg("ffprobe", "-version"),
        check_tool("mediainfo"),
        check_tool("mkvmerge"),
        check_tool("dovi_tool"),
    ]
}

/// Require that a tool is available, returning its path.
///
/// # Errors
///
/// Returns an error if the tool is not found.
pub fn require_tool(name: &str) -> Result<PathBuf> {
    which::which(name).map_err(|_| Error::tool_not_found(name))
}

/// Get the path to a tool, preferring a configured path over PATH lookup.
pub fn get_tool_path(name: &str, config_path: Option<&std::path::Path>) -> Result<PathBuf> {
    if let Some(path) = config_path {
        if path.exists() {
            return Ok(path.to_path_buf());
        }
    }

    require_tool(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_tool_not_found() {
        let info = check_tool("nonexistent_tool_12345");
        assert!(!info.available);
        assert!(info.version.is_none());
        assert!(info.path.is_none());
    }
}
