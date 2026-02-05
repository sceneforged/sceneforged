//! Action configurations that describe what to do when a rule matches.

use serde::{Deserialize, Serialize};
use sf_core::{AudioCodec, Container, StreamType};

/// An action to perform when a rule matches a media file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActionConfig {
    /// Convert Dolby Vision to a different profile.
    DvConvert {
        /// Target DV profile number.
        target_profile: u8,
    },
    /// Remux the file into a different container.
    Remux {
        /// Target container format.
        container: Container,
        /// Whether to keep the original file after remuxing.
        keep_original: bool,
    },
    /// Add a compatibility audio track.
    AddCompatAudio {
        /// Source audio codec to find.
        source_codec: AudioCodec,
        /// Target audio codec to add.
        target_codec: AudioCodec,
    },
    /// Strip tracks from the file.
    StripTracks {
        /// Types of tracks to strip.
        track_types: Vec<StreamType>,
        /// Optional list of languages to target; if `None`, strip all of the given types.
        languages: Option<Vec<String>>,
    },
    /// Execute an external command.
    Exec {
        /// The command to run.
        command: String,
        /// Arguments to pass to the command.
        args: Vec<String>,
    },
    /// Convert to Profile B (H.264 High / AAC-LC stereo MP4).
    ProfileBConvert {
        /// Override CRF value (None = adaptive based on resolution).
        crf: Option<u32>,
        /// Override preset (None = from ConversionConfig).
        preset: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_roundtrip_dv_convert() {
        let action = ActionConfig::DvConvert { target_profile: 8 };
        let json = serde_json::to_string(&action).unwrap();
        let back: ActionConfig = serde_json::from_str(&json).unwrap();
        match back {
            ActionConfig::DvConvert { target_profile } => assert_eq!(target_profile, 8),
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn serde_roundtrip_remux() {
        let action = ActionConfig::Remux {
            container: Container::Mp4,
            keep_original: true,
        };
        let json = serde_json::to_string(&action).unwrap();
        let back: ActionConfig = serde_json::from_str(&json).unwrap();
        match back {
            ActionConfig::Remux {
                container,
                keep_original,
            } => {
                assert_eq!(container, Container::Mp4);
                assert!(keep_original);
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn serde_roundtrip_add_compat_audio() {
        let action = ActionConfig::AddCompatAudio {
            source_codec: AudioCodec::TrueHd,
            target_codec: AudioCodec::Eac3,
        };
        let json = serde_json::to_string(&action).unwrap();
        let back: ActionConfig = serde_json::from_str(&json).unwrap();
        match back {
            ActionConfig::AddCompatAudio {
                source_codec,
                target_codec,
            } => {
                assert_eq!(source_codec, AudioCodec::TrueHd);
                assert_eq!(target_codec, AudioCodec::Eac3);
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn serde_roundtrip_strip_tracks() {
        let action = ActionConfig::StripTracks {
            track_types: vec![StreamType::Subtitle],
            languages: Some(vec!["fra".to_string()]),
        };
        let json = serde_json::to_string(&action).unwrap();
        let back: ActionConfig = serde_json::from_str(&json).unwrap();
        match back {
            ActionConfig::StripTracks {
                track_types,
                languages,
            } => {
                assert_eq!(track_types, vec![StreamType::Subtitle]);
                assert_eq!(languages, Some(vec!["fra".to_string()]));
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn serde_roundtrip_exec() {
        let action = ActionConfig::Exec {
            command: "ffmpeg".to_string(),
            args: vec!["-i".to_string(), "input.mkv".to_string()],
        };
        let json = serde_json::to_string(&action).unwrap();
        let back: ActionConfig = serde_json::from_str(&json).unwrap();
        match back {
            ActionConfig::Exec { command, args } => {
                assert_eq!(command, "ffmpeg");
                assert_eq!(args, vec!["-i", "input.mkv"]);
            }
            _ => panic!("unexpected variant"),
        }
    }
}
