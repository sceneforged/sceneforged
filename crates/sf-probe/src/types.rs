//! Core types for media probe results.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sf_core::{AudioCodec, Container, HdrFormat, VideoCodec};

/// Complete media file information extracted by probing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    /// Path to the probed file.
    pub file_path: PathBuf,
    /// File size in bytes.
    pub file_size: u64,
    /// Container format.
    pub container: Container,
    /// Total duration (if determinable).
    pub duration: Option<Duration>,
    /// Video tracks found in the file.
    pub video_tracks: Vec<VideoTrack>,
    /// Audio tracks found in the file.
    pub audio_tracks: Vec<AudioTrack>,
    /// Subtitle tracks found in the file.
    pub subtitle_tracks: Vec<SubtitleTrack>,
}

impl MediaInfo {
    /// Returns the primary video track.
    ///
    /// Prefers the first track marked as default; falls back to the first track.
    pub fn primary_video(&self) -> Option<&VideoTrack> {
        self.video_tracks
            .iter()
            .find(|t| t.default)
            .or_else(|| self.video_tracks.first())
    }

    /// Returns the primary audio track.
    ///
    /// Prefers the first track marked as default; falls back to the first track.
    pub fn primary_audio(&self) -> Option<&AudioTrack> {
        self.audio_tracks
            .iter()
            .find(|t| t.default)
            .or_else(|| self.audio_tracks.first())
    }
}

/// A video track within a media file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoTrack {
    /// Video codec.
    pub codec: VideoCodec,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Frame rate in frames per second.
    pub frame_rate: Option<f64>,
    /// Bit depth (8, 10, 12).
    pub bit_depth: Option<u8>,
    /// Detected HDR format.
    pub hdr_format: HdrFormat,
    /// Dolby Vision info (if detected).
    pub dolby_vision: Option<DvInfo>,
    /// Whether this is the default track.
    pub default: bool,
    /// Language code (ISO 639-2 or IETF).
    pub language: Option<String>,
}

/// An audio track within a media file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioTrack {
    /// Audio codec.
    pub codec: AudioCodec,
    /// Number of channels.
    pub channels: u32,
    /// Sample rate in Hz.
    pub sample_rate: Option<u32>,
    /// Language code (ISO 639-2 or IETF).
    pub language: Option<String>,
    /// Whether the track contains Dolby Atmos spatial audio.
    pub atmos: bool,
    /// Whether this is the default track.
    pub default: bool,
}

/// A subtitle track within a media file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleTrack {
    /// Subtitle codec/format identifier (e.g. "SRT", "ASS", "PGS").
    pub codec: String,
    /// Language code (ISO 639-2 or IETF).
    pub language: Option<String>,
    /// Whether this is a forced subtitle track.
    pub forced: bool,
    /// Whether this is the default track.
    pub default: bool,
}

/// Dolby Vision information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DvInfo {
    /// Dolby Vision profile (0-10).
    pub profile: u8,
    /// Whether an RPU (Reference Processing Unit) is present.
    pub rpu_present: bool,
    /// Whether an enhancement layer is present.
    pub el_present: bool,
    /// Whether a base layer is present.
    pub bl_present: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_video_prefers_default() {
        let info = MediaInfo {
            file_path: PathBuf::from("/test.mkv"),
            file_size: 1000,
            container: Container::Mkv,
            duration: None,
            video_tracks: vec![
                VideoTrack {
                    codec: VideoCodec::H265,
                    width: 1920,
                    height: 1080,
                    frame_rate: None,
                    bit_depth: None,
                    hdr_format: HdrFormat::Sdr,
                    dolby_vision: None,
                    default: false,
                    language: None,
                },
                VideoTrack {
                    codec: VideoCodec::H264,
                    width: 3840,
                    height: 2160,
                    frame_rate: None,
                    bit_depth: None,
                    hdr_format: HdrFormat::Sdr,
                    dolby_vision: None,
                    default: true,
                    language: None,
                },
            ],
            audio_tracks: vec![],
            subtitle_tracks: vec![],
        };

        let primary = info.primary_video().unwrap();
        assert_eq!(primary.codec, VideoCodec::H264);
        assert_eq!(primary.width, 3840);
    }

    #[test]
    fn primary_video_falls_back_to_first() {
        let info = MediaInfo {
            file_path: PathBuf::from("/test.mkv"),
            file_size: 1000,
            container: Container::Mkv,
            duration: None,
            video_tracks: vec![VideoTrack {
                codec: VideoCodec::H265,
                width: 1920,
                height: 1080,
                frame_rate: None,
                bit_depth: None,
                hdr_format: HdrFormat::Sdr,
                dolby_vision: None,
                default: false,
                language: None,
            }],
            audio_tracks: vec![],
            subtitle_tracks: vec![],
        };

        let primary = info.primary_video().unwrap();
        assert_eq!(primary.codec, VideoCodec::H265);
    }

    #[test]
    fn primary_video_empty() {
        let info = MediaInfo {
            file_path: PathBuf::from("/test.mkv"),
            file_size: 1000,
            container: Container::Mkv,
            duration: None,
            video_tracks: vec![],
            audio_tracks: vec![],
            subtitle_tracks: vec![],
        };

        assert!(info.primary_video().is_none());
    }

    #[test]
    fn primary_audio_prefers_default() {
        let info = MediaInfo {
            file_path: PathBuf::from("/test.mkv"),
            file_size: 1000,
            container: Container::Mkv,
            duration: None,
            video_tracks: vec![],
            audio_tracks: vec![
                AudioTrack {
                    codec: AudioCodec::Aac,
                    channels: 2,
                    sample_rate: Some(48000),
                    language: None,
                    atmos: false,
                    default: false,
                },
                AudioTrack {
                    codec: AudioCodec::Eac3,
                    channels: 6,
                    sample_rate: Some(48000),
                    language: None,
                    atmos: false,
                    default: true,
                },
            ],
            subtitle_tracks: vec![],
        };

        let primary = info.primary_audio().unwrap();
        assert_eq!(primary.codec, AudioCodec::Eac3);
        assert_eq!(primary.channels, 6);
    }

    #[test]
    fn media_info_serde_roundtrip() {
        let info = MediaInfo {
            file_path: PathBuf::from("/test.mkv"),
            file_size: 42,
            container: Container::Mkv,
            duration: Some(Duration::from_secs(120)),
            video_tracks: vec![VideoTrack {
                codec: VideoCodec::H265,
                width: 3840,
                height: 2160,
                frame_rate: Some(23.976),
                bit_depth: Some(10),
                hdr_format: HdrFormat::Hdr10,
                dolby_vision: Some(DvInfo {
                    profile: 7,
                    rpu_present: true,
                    el_present: false,
                    bl_present: true,
                }),
                default: true,
                language: Some("eng".to_string()),
            }],
            audio_tracks: vec![AudioTrack {
                codec: AudioCodec::TrueHd,
                channels: 8,
                sample_rate: Some(48000),
                language: Some("eng".to_string()),
                atmos: true,
                default: true,
            }],
            subtitle_tracks: vec![SubtitleTrack {
                codec: "PGS".to_string(),
                language: Some("eng".to_string()),
                forced: false,
                default: true,
            }],
        };

        let json = serde_json::to_string(&info).unwrap();
        let back: MediaInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.file_size, 42);
        assert_eq!(back.video_tracks.len(), 1);
        assert_eq!(back.video_tracks[0].width, 3840);
        assert_eq!(back.audio_tracks[0].channels, 8);
    }
}
