//! Media information types.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Information about a media file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    /// Path to the media file.
    pub file_path: PathBuf,
    /// File size in bytes.
    pub file_size: u64,
    /// Container format (e.g., "Matroska", "MPEG-4").
    pub container: String,
    /// Duration of the media.
    pub duration: Option<Duration>,
    /// Video tracks in the file.
    pub video_tracks: Vec<VideoTrack>,
    /// Audio tracks in the file.
    pub audio_tracks: Vec<AudioTrack>,
    /// Subtitle tracks in the file.
    pub subtitle_tracks: Vec<SubtitleTrack>,
}

/// Information about a video track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoTrack {
    /// Track index.
    pub index: u32,
    /// Video codec (e.g., "HEVC", "AVC").
    pub codec: String,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Frame rate in FPS.
    pub frame_rate: Option<f64>,
    /// Bit depth (e.g., 8, 10, 12).
    pub bit_depth: Option<u8>,
    /// HDR format if present.
    pub hdr_format: Option<HdrFormat>,
    /// Dolby Vision info if present.
    pub dolby_vision: Option<DolbyVisionInfo>,
}

/// HDR format types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HdrFormat {
    /// Standard Dynamic Range (not HDR).
    Sdr,
    /// HDR10 (static metadata).
    Hdr10,
    /// HDR10+ (dynamic metadata).
    Hdr10Plus,
    /// Dolby Vision.
    DolbyVision,
    /// Hybrid Log-Gamma.
    Hlg,
}

/// Dolby Vision metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DolbyVisionInfo {
    /// Dolby Vision profile (e.g., 5, 7, 8).
    pub profile: u8,
    /// Dolby Vision level.
    pub level: Option<u8>,
    /// Whether RPU (Reference Processing Unit) is present.
    pub rpu_present: bool,
    /// Whether Enhancement Layer is present.
    pub el_present: bool,
    /// Whether Base Layer is present.
    pub bl_present: bool,
    /// Base layer compatibility ID.
    pub bl_compatibility_id: Option<u8>,
}

/// Information about an audio track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioTrack {
    /// Track index.
    pub index: u32,
    /// Audio codec (e.g., "AAC", "TrueHD", "E-AC-3").
    pub codec: String,
    /// Number of channels.
    pub channels: u32,
    /// Sample rate in Hz.
    pub sample_rate: Option<u32>,
    /// Language code (e.g., "eng", "spa").
    pub language: Option<String>,
    /// Track title.
    pub title: Option<String>,
    /// Whether this is the default track.
    pub default: bool,
    /// Whether this track contains Dolby Atmos.
    pub atmos: bool,
}

/// Information about a subtitle track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleTrack {
    /// Track index.
    pub index: u32,
    /// Subtitle format (e.g., "SRT", "PGS", "ASS").
    pub codec: String,
    /// Language code (e.g., "eng", "spa").
    pub language: Option<String>,
    /// Track title.
    pub title: Option<String>,
    /// Whether this is the default track.
    pub default: bool,
    /// Whether this is a forced track.
    pub forced: bool,
}

impl MediaInfo {
    /// Get the primary (first) video track.
    pub fn primary_video(&self) -> Option<&VideoTrack> {
        self.video_tracks.first()
    }

    /// Get a human-readable resolution name.
    pub fn resolution_name(&self) -> Option<&'static str> {
        self.primary_video().map(|v| match (v.width, v.height) {
            (w, h) if w >= 3840 || h >= 2160 => "4K",
            (w, h) if w >= 1920 || h >= 1080 => "1080p",
            (w, h) if w >= 1280 || h >= 720 => "720p",
            (w, h) if w >= 720 || h >= 480 => "480p",
            _ => "SD",
        })
    }

    /// Check if any video track has Dolby Vision.
    pub fn has_dolby_vision(&self) -> bool {
        self.video_tracks.iter().any(|v| v.dolby_vision.is_some())
    }

    /// Get the Dolby Vision profile if present.
    pub fn dolby_vision_profile(&self) -> Option<u8> {
        self.video_tracks
            .iter()
            .filter_map(|v| v.dolby_vision.as_ref())
            .map(|dv| dv.profile)
            .next()
    }

    /// Check if any video track has HDR.
    pub fn has_hdr(&self) -> bool {
        self.video_tracks.iter().any(|v| {
            matches!(
                v.hdr_format,
                Some(
                    HdrFormat::Hdr10
                        | HdrFormat::Hdr10Plus
                        | HdrFormat::DolbyVision
                        | HdrFormat::Hlg
                )
            )
        })
    }

    /// Check if any audio track has Dolby Atmos.
    pub fn has_atmos(&self) -> bool {
        self.audio_tracks.iter().any(|a| a.atmos)
    }
}

impl std::fmt::Display for HdrFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HdrFormat::Sdr => write!(f, "SDR"),
            HdrFormat::Hdr10 => write!(f, "HDR10"),
            HdrFormat::Hdr10Plus => write!(f, "HDR10+"),
            HdrFormat::DolbyVision => write!(f, "Dolby Vision"),
            HdrFormat::Hlg => write!(f, "HLG"),
        }
    }
}
