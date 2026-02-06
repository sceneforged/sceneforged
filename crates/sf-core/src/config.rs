//! Application configuration types.
//!
//! The top-level [`Config`] struct is deserialized from JSON and carries all
//! sub-configs for server, auth, tools, conversion, etc. Every section
//! defaults sensibly so a completely empty `{}` file is valid.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::Error;

// ---------------------------------------------------------------------------
// Top-level Config
// ---------------------------------------------------------------------------

/// Root application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
    pub auth: AuthConfig,
    pub watch: WatchConfig,
    #[serde(default)]
    pub arrs: Vec<ArrConfig>,
    #[serde(default)]
    pub jellyfins: Vec<JellyfinConfig>,
    pub tools: ToolsConfig,
    pub conversion: ConversionConfig,
    pub metadata: MetadataConfig,
    pub images: ImageConfig,
    pub webhook_security: WebhookSecurityConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            auth: AuthConfig::default(),
            watch: WatchConfig::default(),
            arrs: Vec::new(),
            jellyfins: Vec::new(),
            tools: ToolsConfig::default(),
            conversion: ConversionConfig::default(),
            metadata: MetadataConfig::default(),
            images: ImageConfig::default(),
            webhook_security: WebhookSecurityConfig::default(),
        }
    }
}

impl Config {
    /// Deserialize a `Config` from a JSON string.
    ///
    /// This is intentionally string-based so the caller can read the file
    /// however it sees fit (async, embedded, etc.).
    pub fn from_json(json_str: &str) -> Result<Self> {
        serde_json::from_str(json_str)
            .map_err(|e| Error::Validation(format!("config parse error: {e}")))
    }

    /// Load configuration from a file path, falling back to defaults if the
    /// path is `None` or the file does not exist.
    pub fn load_or_default(path: Option<&Path>) -> Self {
        let Some(path) = path else {
            return Self::default();
        };

        match std::fs::read_to_string(path) {
            Ok(contents) => Self::from_json(&contents).unwrap_or_else(|e| {
                tracing::warn!("Failed to parse config file {}: {e}", path.display());
                Self::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!("No config file at {}; using defaults", path.display());
                Self::default()
            }
            Err(e) => {
                tracing::warn!("Failed to read config file {}: {e}", path.display());
                Self::default()
            }
        }
    }

    /// Return a list of validation warnings (non-fatal issues).
    pub fn validate(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        if self.server.port == 0 {
            warnings.push("server.port is 0; a random port will be assigned".into());
        }

        if self.auth.enabled {
            if self.auth.api_key.is_none() && self.auth.username.is_none() {
                warnings.push(
                    "auth is enabled but neither api_key nor username is set".into(),
                );
            }
            if self.auth.username.is_some() && self.auth.password_hash.is_none() {
                warnings.push(
                    "auth username is set but password_hash is missing".into(),
                );
            }
        }

        for (i, arr) in self.arrs.iter().enumerate() {
            if arr.url.is_empty() {
                warnings.push(format!("arrs[{i}].url is empty"));
            }
            if arr.api_key.is_empty() {
                warnings.push(format!("arrs[{i}].api_key is empty"));
            }
        }

        for (i, jf) in self.jellyfins.iter().enumerate() {
            if jf.url.is_empty() {
                warnings.push(format!("jellyfins[{i}].url is empty"));
            }
            if jf.api_key.is_empty() {
                warnings.push(format!("jellyfins[{i}].api_key is empty"));
            }
        }

        if self.webhook_security.signature_verification
            && self.webhook_security.signature_secret.is_none()
        {
            warnings.push(
                "webhook signature_verification is enabled but no signature_secret is set".into(),
            );
        }

        if let Some(ref hw) = self.conversion.hw_accel {
            let valid = ["none", "videotoolbox", "nvenc", "vaapi", "qsv"];
            if !valid.contains(&hw.as_str()) {
                warnings.push(format!(
                    "conversion.hw_accel '{}' is not a recognized method (valid: {})",
                    hw,
                    valid.join(", ")
                ));
            }
        }

        warnings
    }
}

// ---------------------------------------------------------------------------
// Sub-configs
// ---------------------------------------------------------------------------

/// HTTP server settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub static_dir: Option<PathBuf>,
    pub db_path: PathBuf,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".into(),
            port: 8080,
            static_dir: Some(PathBuf::from("/app/static")),
            db_path: PathBuf::from("/data/sceneforged.db"),
        }
    }
}

/// Authentication settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    pub enabled: bool,
    pub api_key: Option<String>,
    pub username: Option<String>,
    pub password_hash: Option<String>,
    #[serde(default = "default_session_timeout")]
    pub session_timeout_hours: u64,
}

fn default_session_timeout() -> u64 {
    24
}

/// File-system watcher settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WatchConfig {
    pub enabled: bool,
    pub paths: Vec<PathBuf>,
    #[serde(default = "default_settle_time")]
    pub settle_time_secs: u64,
    pub extensions: Vec<String>,
}

fn default_settle_time() -> u64 {
    30
}

/// Configuration for an *arr (Radarr / Sonarr) integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub arr_type: String,
    pub url: String,
    pub api_key: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub auto_rescan: bool,
    #[serde(default)]
    pub auto_rename: bool,
}

fn default_true() -> bool {
    true
}

/// Configuration for a Jellyfin server connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JellyfinConfig {
    pub name: String,
    pub url: String,
    pub api_key: String,
    #[serde(default)]
    pub enabled: bool,
}

/// Paths to external CLI tools.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolsConfig {
    pub ffmpeg_path: Option<PathBuf>,
    pub ffprobe_path: Option<PathBuf>,
    pub mediainfo_path: Option<PathBuf>,
    pub mkvmerge_path: Option<PathBuf>,
    pub dovi_tool_path: Option<PathBuf>,
}

/// Video conversion defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConversionConfig {
    pub auto_convert_on_scan: bool,
    pub auto_convert_dv_p7_to_p8: bool,
    #[serde(default = "default_video_crf")]
    pub video_crf: u32,
    #[serde(default = "default_video_preset")]
    pub video_preset: String,
    #[serde(default = "default_audio_bitrate")]
    pub audio_bitrate: String,
    #[serde(default = "default_adaptive_crf")]
    pub adaptive_crf: bool,
    /// Hardware acceleration method (none, videotoolbox, nvenc, vaapi, qsv).
    /// When set to a supported value, ffmpeg will use the corresponding
    /// hardware decoder and encoder instead of the default libx264.
    #[serde(default)]
    pub hw_accel: Option<String>,
}

fn default_video_crf() -> u32 {
    15
}
fn default_video_preset() -> String {
    "slow".into()
}
fn default_audio_bitrate() -> String {
    "256k".into()
}
fn default_adaptive_crf() -> bool {
    true
}

impl Default for ConversionConfig {
    fn default() -> Self {
        Self {
            auto_convert_on_scan: false,
            auto_convert_dv_p7_to_p8: false,
            video_crf: default_video_crf(),
            video_preset: default_video_preset(),
            audio_bitrate: default_audio_bitrate(),
            adaptive_crf: default_adaptive_crf(),
            hw_accel: None,
        }
    }
}

/// Metadata enrichment settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MetadataConfig {
    pub auto_enrich: bool,
    pub tmdb_api_key: Option<String>,
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_language() -> String {
    "en-US".into()
}

impl Default for MetadataConfig {
    fn default() -> Self {
        Self {
            auto_enrich: true,
            tmdb_api_key: None,
            language: default_language(),
        }
    }
}

/// Image storage settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageConfig {
    pub storage_dir: PathBuf,
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self {
            storage_dir: PathBuf::from("./data/images"),
        }
    }
}

/// Webhook signature verification settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WebhookSecurityConfig {
    pub signature_verification: bool,
    pub signature_secret: Option<String>,
    pub allowed_ips: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let cfg = Config::default();
        assert_eq!(cfg.server.host, "0.0.0.0");
        assert_eq!(cfg.server.port, 8080);
        assert_eq!(cfg.server.static_dir, Some(PathBuf::from("/app/static")));
        assert!(!cfg.auth.enabled);
        assert_eq!(cfg.conversion.video_crf, 15);
        assert_eq!(cfg.conversion.video_preset, "slow");
        assert_eq!(cfg.images.storage_dir, PathBuf::from("./data/images"));
    }

    #[test]
    fn default_config_no_warnings() {
        let cfg = Config::default();
        let warnings = cfg.validate();
        assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);
    }

    #[test]
    fn auth_enabled_without_credentials_warns() {
        let mut cfg = Config::default();
        cfg.auth.enabled = true;
        let warnings = cfg.validate();
        assert!(warnings.iter().any(|w| w.contains("api_key")));
    }

    #[test]
    fn username_without_password_warns() {
        let mut cfg = Config::default();
        cfg.auth.enabled = true;
        cfg.auth.username = Some("admin".into());
        let warnings = cfg.validate();
        assert!(warnings.iter().any(|w| w.contains("password_hash")));
    }

    #[test]
    fn parse_json_config() {
        let json = r#"{"server": {"port": 9090}}"#;
        let cfg = Config::from_json(json).unwrap();
        assert_eq!(cfg.server.port, 9090);
    }

    #[test]
    fn parse_empty_json_uses_defaults() {
        let cfg = Config::from_json("{}").unwrap();
        assert_eq!(cfg.server.host, "0.0.0.0");
        assert_eq!(cfg.server.port, 8080);
    }

    #[test]
    fn load_or_default_with_none() {
        let cfg = Config::load_or_default(None);
        assert_eq!(cfg.server.port, 8080);
    }

    #[test]
    fn load_or_default_with_missing_file() {
        let cfg = Config::load_or_default(Some(Path::new("/nonexistent/config.json")));
        assert_eq!(cfg.server.port, 8080);
    }

    #[test]
    fn webhook_signature_without_secret_warns() {
        let mut cfg = Config::default();
        cfg.webhook_security.signature_verification = true;
        let warnings = cfg.validate();
        assert!(warnings.iter().any(|w| w.contains("signature_secret")));
    }
}
