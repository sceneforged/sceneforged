//! External tool detection and management.
//!
//! The [`ToolRegistry`] discovers and caches the locations of external CLI
//! tools (ffmpeg, ffprobe, mediainfo, mkvmerge, mkvextract, dovi_tool) and
//! provides lookup methods for the rest of the crate.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Default tool timeout: 5 minutes.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

/// Known tool names that the registry manages.
const KNOWN_TOOLS: &[&str] = &[
    "ffmpeg",
    "ffprobe",
    "mediainfo",
    "mkvmerge",
    "mkvextract",
    "dovi_tool",
];

/// Configuration for a single external tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    /// Human-readable tool name (e.g. "ffmpeg").
    pub name: String,
    /// Resolved path to the executable.
    pub path: PathBuf,
    /// Optional minimum version requirement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_version: Option<semver::VersionReq>,
    /// Maximum execution time before the tool is killed.
    #[serde(
        default = "default_timeout",
        with = "duration_secs",
        skip_serializing_if = "is_default_timeout"
    )]
    pub timeout: Duration,
}

fn default_timeout() -> Duration {
    DEFAULT_TIMEOUT
}

fn is_default_timeout(d: &Duration) -> bool {
    *d == DEFAULT_TIMEOUT
}

/// Serde helpers to (de)serialize `Duration` as whole seconds.
mod duration_secs {
    use serde::{self, Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

/// Availability information for a tool, returned by [`ToolRegistry::check_all`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    /// Tool name.
    pub name: String,
    /// Whether the tool was found.
    pub available: bool,
    /// Version string (first line of `--version` output), if available.
    pub version: Option<String>,
    /// Resolved path to the executable.
    pub path: Option<PathBuf>,
}

/// Registry holding discovered tool configurations.
#[derive(Debug, Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, ToolConfig>,
}

impl ToolRegistry {
    /// Discover tools by searching `PATH` (or using overrides from config).
    ///
    /// For each known tool, if the [`sf_core::config::ToolsConfig`] supplies a
    /// custom path **and** that path exists, it is used directly.  Otherwise
    /// [`which::which`] is used to locate the tool in `PATH`.  Tools that are
    /// not found are silently omitted from the registry.
    pub fn discover(tools_config: &sf_core::config::ToolsConfig) -> Self {
        let mut tools = HashMap::new();

        for &name in KNOWN_TOOLS {
            let custom_path = match name {
                "ffmpeg" => tools_config.ffmpeg_path.as_deref(),
                "ffprobe" => tools_config.ffprobe_path.as_deref(),
                "mediainfo" => tools_config.mediainfo_path.as_deref(),
                "mkvmerge" => tools_config.mkvmerge_path.as_deref(),
                "dovi_tool" => tools_config.dovi_tool_path.as_deref(),
                _ => None,
            };

            let resolved = if let Some(p) = custom_path {
                if p.exists() {
                    Some(p.to_path_buf())
                } else {
                    // Custom path does not exist; fall back to PATH.
                    which::which(name).ok()
                }
            } else {
                which::which(name).ok()
            };

            if let Some(path) = resolved {
                tools.insert(
                    name.to_string(),
                    ToolConfig {
                        name: name.to_string(),
                        path,
                        min_version: None,
                        timeout: DEFAULT_TIMEOUT,
                    },
                );
            }
        }

        Self { tools }
    }

    /// Return a reference to the [`ToolConfig`] for the given tool, or an
    /// [`sf_core::Error::Tool`] if the tool was not found during discovery.
    pub fn require(&self, name: &str) -> sf_core::Result<&ToolConfig> {
        self.tools.get(name).ok_or_else(|| {
            sf_core::Error::Tool {
                tool: name.to_string(),
                message: format!("{name} not found; is it installed and in PATH?"),
            }
        })
    }

    /// Check all known tools and return availability information.
    pub fn check_all(&self) -> Vec<ToolInfo> {
        KNOWN_TOOLS
            .iter()
            .map(|&name| {
                if let Some(cfg) = self.tools.get(name) {
                    let version = detect_version(name, &cfg.path);
                    ToolInfo {
                        name: name.to_string(),
                        available: true,
                        version,
                        path: Some(cfg.path.clone()),
                    }
                } else {
                    ToolInfo {
                        name: name.to_string(),
                        available: false,
                        version: None,
                        path: None,
                    }
                }
            })
            .collect()
    }

    /// Iterate over all registered tool configs.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &ToolConfig)> {
        self.tools.iter()
    }
}

/// Run `<tool> --version` (or `-version` for ffmpeg/ffprobe) and return the
/// first line of stdout.
fn detect_version(name: &str, path: &PathBuf) -> Option<String> {
    let version_arg = match name {
        "ffmpeg" | "ffprobe" => "-version",
        _ => "--version",
    };

    let output = std::process::Command::new(path)
        .arg(version_arg)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_core::config::ToolsConfig;

    #[test]
    fn discover_with_default_config() {
        let cfg = ToolsConfig::default();
        let registry = ToolRegistry::discover(&cfg);
        // We cannot guarantee any tool is installed in CI,
        // but the call itself must not panic.
        let _ = registry.check_all();
    }

    #[test]
    fn require_missing_tool_returns_error() {
        let cfg = ToolsConfig::default();
        let registry = ToolRegistry::discover(&cfg);
        let result = registry.require("nonexistent_tool_xyz");
        assert!(result.is_err());
    }

    #[test]
    fn check_all_returns_known_tools() {
        let cfg = ToolsConfig::default();
        let registry = ToolRegistry::discover(&cfg);
        let infos = registry.check_all();
        let names: Vec<&str> = infos.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"ffmpeg"));
        assert!(names.contains(&"ffprobe"));
        assert!(names.contains(&"mediainfo"));
        assert!(names.contains(&"mkvmerge"));
        assert!(names.contains(&"mkvextract"));
        assert!(names.contains(&"dovi_tool"));
    }

    #[test]
    fn tool_config_serialization() {
        let cfg = ToolConfig {
            name: "ffmpeg".to_string(),
            path: PathBuf::from("/usr/bin/ffmpeg"),
            min_version: None,
            timeout: DEFAULT_TIMEOUT,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains("ffmpeg"));
        let back: ToolConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "ffmpeg");
    }
}
