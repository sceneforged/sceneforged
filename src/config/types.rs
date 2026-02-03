use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,

    #[serde(default)]
    pub watch: WatchConfig,

    #[serde(default)]
    pub arrs: Vec<ArrConfig>,

    #[serde(default)]
    pub jellyfins: Vec<JellyfinConfig>,

    #[serde(default)]
    pub rules: Vec<Rule>,

    #[serde(default)]
    pub tools: ToolsConfig,

    #[serde(default)]
    pub conversion: ConversionConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default)]
    pub static_dir: Option<PathBuf>,

    #[serde(default)]
    pub auth: AuthConfig,

    #[serde(default)]
    pub webhook_security: WebhookSecurityConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AuthConfig {
    /// Enable authentication for API and UI
    #[serde(default)]
    pub enabled: bool,

    /// API key for programmatic access (used with Authorization: Bearer header)
    #[serde(default)]
    pub api_key: Option<String>,

    /// Username for web UI login
    #[serde(default)]
    pub username: Option<String>,

    /// Bcrypt hash of the password (generate with `sceneforged hash-password`)
    #[serde(default)]
    pub password_hash: Option<String>,

    /// Session timeout in hours (default: 24)
    #[serde(default = "default_session_timeout")]
    pub session_timeout_hours: u64,
}

fn default_session_timeout() -> u64 {
    24
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct WebhookSecurityConfig {
    /// Enable webhook signature verification
    #[serde(default)]
    pub signature_verification: bool,

    /// Shared secret for HMAC-SHA256 signature verification
    #[serde(default)]
    pub signature_secret: Option<String>,

    /// Allowed IP addresses or CIDR ranges (empty = allow all)
    #[serde(default)]
    pub allowed_ips: Vec<String>,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}
fn default_port() -> u16 {
    8080
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            static_dir: None,
            auth: AuthConfig::default(),
            webhook_security: WebhookSecurityConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct WatchConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub paths: Vec<PathBuf>,

    #[serde(default = "default_settle_time")]
    pub settle_time_secs: u64,

    #[serde(default)]
    pub extensions: Vec<String>,
}

fn default_settle_time() -> u64 {
    30
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArrConfig {
    pub name: String,

    #[serde(rename = "type")]
    pub arr_type: ArrType,

    pub url: String,

    pub api_key: String,

    #[serde(default)]
    pub enabled: bool,

    /// Trigger a rescan in the Arr after job completion (default: true)
    #[serde(default = "default_auto_rescan")]
    pub auto_rescan: bool,

    /// Trigger a rename in the Arr after job completion (default: false)
    #[serde(default)]
    pub auto_rename: bool,
}

fn default_auto_rescan() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ArrType {
    Radarr,
    Sonarr,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JellyfinConfig {
    pub name: String,

    pub url: String,

    pub api_key: String,

    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rule {
    pub name: String,

    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub priority: i32,

    #[serde(rename = "match")]
    pub match_conditions: MatchConditions,

    pub actions: Vec<Action>,

    #[serde(skip)]
    pub normalized: Option<NormalizedMatchConditions>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MatchConditions {
    #[serde(default)]
    pub codecs: Vec<String>,

    #[serde(default)]
    pub containers: Vec<String>,

    #[serde(default)]
    pub hdr_formats: Vec<String>,

    #[serde(default)]
    pub dolby_vision_profiles: Vec<u8>,

    #[serde(default)]
    pub min_resolution: Option<Resolution>,

    #[serde(default)]
    pub max_resolution: Option<Resolution>,

    #[serde(default)]
    pub audio_codecs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HdrFormatMatch {
    Sdr,
    Hdr10,
    Hdr10Plus,
    DolbyVision,
    Hlg,
}

impl std::str::FromStr for HdrFormatMatch {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "sdr" => Ok(Self::Sdr),
            "hdr10" => Ok(Self::Hdr10),
            "hdr10+" | "hdr10plus" => Ok(Self::Hdr10Plus),
            "dolbyvision" | "dolby_vision" | "dv" => Ok(Self::DolbyVision),
            "hlg" => Ok(Self::Hlg),
            _ => Err(format!("Unknown HDR format: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct NormalizedMatchConditions {
    pub codecs: Vec<String>,
    pub containers: Vec<String>,
    pub hdr_formats: Vec<HdrFormatMatch>,
    pub dolby_vision_profiles: Vec<u8>,
    pub min_resolution: Option<Resolution>,
    pub max_resolution: Option<Resolution>,
    pub audio_codecs: Vec<String>,
}

impl From<&MatchConditions> for NormalizedMatchConditions {
    fn from(mc: &MatchConditions) -> Self {
        Self {
            codecs: mc.codecs.iter().map(|s| s.to_lowercase()).collect(),
            containers: mc.containers.iter().map(|s| s.to_lowercase()).collect(),
            hdr_formats: mc
                .hdr_formats
                .iter()
                .filter_map(|s| s.parse().ok())
                .collect(),
            dolby_vision_profiles: mc.dolby_vision_profiles.clone(),
            min_resolution: mc.min_resolution.clone(),
            max_resolution: mc.max_resolution.clone(),
            audio_codecs: mc.audio_codecs.iter().map(|s| s.to_lowercase()).collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    DvConvert {
        target_profile: u8,
    },
    Remux {
        container: String,
        #[serde(default)]
        keep_original: bool,
    },
    AddCompatAudio {
        source_codec: String,
        target_codec: String,
    },
    StripTracks {
        #[serde(default)]
        track_types: Vec<String>,
        #[serde(default)]
        languages: Vec<String>,
    },
    Exec {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConversionConfig {
    /// Automatically queue Profile C files for Profile B conversion on scan
    #[serde(default)]
    pub auto_convert_on_scan: bool,

    /// Auto-convert DV Profile 7 files to Profile 8 on import
    #[serde(default)]
    pub auto_convert_dv_p7_to_p8: bool,

    /// Video CRF for Profile B conversion (lower = higher quality, default: 15)
    #[serde(default = "default_video_crf")]
    pub video_crf: u32,

    /// Video encoding preset (default: "slow")
    #[serde(default = "default_video_preset")]
    pub video_preset: String,

    /// Audio bitrate for Profile B conversion (default: "256k")
    #[serde(default = "default_audio_bitrate")]
    pub audio_bitrate: String,

    /// Whether to use adaptive CRF based on source resolution (default: true)
    #[serde(default = "default_adaptive_crf")]
    pub adaptive_crf: bool,
}

fn default_video_crf() -> u32 {
    15
}

fn default_video_preset() -> String {
    "slow".to_string()
}

fn default_audio_bitrate() -> String {
    "256k".to_string()
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
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ToolsConfig {
    #[serde(default)]
    pub ffmpeg_path: Option<PathBuf>,

    #[serde(default)]
    pub ffprobe_path: Option<PathBuf>,

    #[serde(default)]
    pub mediainfo_path: Option<PathBuf>,

    #[serde(default)]
    pub mkvmerge_path: Option<PathBuf>,

    #[serde(default)]
    pub dovi_tool_path: Option<PathBuf>,
}
