//! Leaf conditions that evaluate against [`MediaInfo`].
//!
//! Each [`Condition`] variant checks a single aspect of a media file.
//! Conditions are composed into expression trees via [`Expr`](crate::Expr).

use serde::{Deserialize, Serialize};
use sf_core::{AudioCodec, Container, HdrFormat, VideoCodec};
use sf_probe::MediaInfo;

/// A leaf condition that evaluates a single property of a media file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Condition {
    /// Matches if the primary video track codec is in the given list.
    Codec(Vec<VideoCodec>),
    /// Matches if the container format is in the given list.
    Container(Vec<Container>),
    /// Matches if the primary video track HDR format is in the given list.
    HdrFormat(Vec<HdrFormat>),
    /// Matches if the Dolby Vision profile is in the given list.
    DolbyVisionProfile(Vec<u8>),
    /// Matches if the primary video resolution is >= both width and height.
    MinResolution { width: u32, height: u32 },
    /// Matches if the primary video resolution is <= both width and height.
    MaxResolution { width: u32, height: u32 },
    /// Matches if any audio track codec is in the given list.
    AudioCodec(Vec<AudioCodec>),
    /// Matches if any audio track has atmos equal to the given value.
    HasAtmos(bool),
    /// Matches if the primary video bit depth is >= the given value.
    MinBitDepth(u8),
    /// Matches on file extension (case-insensitive).
    FileExtension(Vec<String>),
}

impl Condition {
    /// Evaluate this condition against the given media info.
    pub fn evaluate(&self, info: &MediaInfo) -> bool {
        match self {
            Condition::Codec(codecs) => {
                if let Some(video) = info.primary_video() {
                    codecs.contains(&video.codec)
                } else {
                    false
                }
            }
            Condition::Container(containers) => containers.contains(&info.container),
            Condition::HdrFormat(formats) => {
                if let Some(video) = info.primary_video() {
                    formats.contains(&video.hdr_format)
                } else {
                    false
                }
            }
            Condition::DolbyVisionProfile(profiles) => info.video_tracks.iter().any(|track| {
                if let Some(ref dv) = track.dolby_vision {
                    profiles.contains(&dv.profile)
                } else {
                    false
                }
            }),
            Condition::MinResolution { width, height } => {
                if let Some(video) = info.primary_video() {
                    video.width >= *width && video.height >= *height
                } else {
                    false
                }
            }
            Condition::MaxResolution { width, height } => {
                if let Some(video) = info.primary_video() {
                    video.width <= *width && video.height <= *height
                } else {
                    false
                }
            }
            Condition::AudioCodec(codecs) => info
                .audio_tracks
                .iter()
                .any(|track| codecs.contains(&track.codec)),
            Condition::HasAtmos(value) => {
                info.audio_tracks.iter().any(|track| track.atmos == *value)
            }
            Condition::MinBitDepth(min_depth) => {
                if let Some(video) = info.primary_video() {
                    video.bit_depth.map_or(false, |bd| bd >= *min_depth)
                } else {
                    false
                }
            }
            Condition::FileExtension(extensions) => {
                let ext = info
                    .file_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase());
                if let Some(ext) = ext {
                    extensions.iter().any(|e| e.to_lowercase() == ext)
                } else {
                    false
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_probe::{AudioTrack, DvInfo, VideoTrack};
    use std::path::PathBuf;

    fn make_test_info() -> MediaInfo {
        MediaInfo {
            file_path: PathBuf::from("/test/movie.mkv"),
            file_size: 1024 * 1024 * 1024,
            container: Container::Mkv,
            duration: None,
            video_tracks: vec![VideoTrack {
                codec: VideoCodec::H265,
                width: 3840,
                height: 2160,
                frame_rate: Some(23.976),
                bit_depth: Some(10),
                hdr_format: HdrFormat::DolbyVision,
                dolby_vision: Some(DvInfo {
                    profile: 7,
                    rpu_present: true,
                    el_present: true,
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
            subtitle_tracks: vec![],
        }
    }

    #[test]
    fn codec_matches() {
        let info = make_test_info();
        assert!(Condition::Codec(vec![VideoCodec::H265]).evaluate(&info));
        assert!(Condition::Codec(vec![VideoCodec::H264, VideoCodec::H265]).evaluate(&info));
        assert!(!Condition::Codec(vec![VideoCodec::H264]).evaluate(&info));
    }

    #[test]
    fn container_matches() {
        let info = make_test_info();
        assert!(Condition::Container(vec![Container::Mkv]).evaluate(&info));
        assert!(!Condition::Container(vec![Container::Mp4]).evaluate(&info));
    }

    #[test]
    fn hdr_format_matches() {
        let info = make_test_info();
        assert!(Condition::HdrFormat(vec![HdrFormat::DolbyVision]).evaluate(&info));
        assert!(!Condition::HdrFormat(vec![HdrFormat::Hdr10]).evaluate(&info));
    }

    #[test]
    fn dolby_vision_profile_matches() {
        let info = make_test_info();
        assert!(Condition::DolbyVisionProfile(vec![7]).evaluate(&info));
        assert!(Condition::DolbyVisionProfile(vec![7, 8]).evaluate(&info));
        assert!(!Condition::DolbyVisionProfile(vec![8]).evaluate(&info));
    }

    #[test]
    fn min_resolution_matches() {
        let info = make_test_info();
        assert!(Condition::MinResolution {
            width: 3840,
            height: 2160
        }
        .evaluate(&info));
        assert!(Condition::MinResolution {
            width: 1920,
            height: 1080
        }
        .evaluate(&info));
        assert!(!Condition::MinResolution {
            width: 7680,
            height: 4320
        }
        .evaluate(&info));
    }

    #[test]
    fn max_resolution_matches() {
        let info = make_test_info();
        assert!(Condition::MaxResolution {
            width: 3840,
            height: 2160
        }
        .evaluate(&info));
        assert!(Condition::MaxResolution {
            width: 7680,
            height: 4320
        }
        .evaluate(&info));
        assert!(!Condition::MaxResolution {
            width: 1920,
            height: 1080
        }
        .evaluate(&info));
    }

    #[test]
    fn audio_codec_matches() {
        let info = make_test_info();
        assert!(Condition::AudioCodec(vec![AudioCodec::TrueHd]).evaluate(&info));
        assert!(!Condition::AudioCodec(vec![AudioCodec::Aac]).evaluate(&info));
    }

    #[test]
    fn has_atmos_matches() {
        let info = make_test_info();
        assert!(Condition::HasAtmos(true).evaluate(&info));
        assert!(!Condition::HasAtmos(false).evaluate(&info));
    }

    #[test]
    fn min_bit_depth_matches() {
        let info = make_test_info();
        assert!(Condition::MinBitDepth(10).evaluate(&info));
        assert!(Condition::MinBitDepth(8).evaluate(&info));
        assert!(!Condition::MinBitDepth(12).evaluate(&info));
    }

    #[test]
    fn file_extension_matches() {
        let info = make_test_info();
        assert!(Condition::FileExtension(vec!["mkv".to_string()]).evaluate(&info));
        assert!(Condition::FileExtension(vec!["MKV".to_string()]).evaluate(&info));
        assert!(!Condition::FileExtension(vec!["mp4".to_string()]).evaluate(&info));
    }

    #[test]
    fn no_video_tracks_returns_false_for_video_conditions() {
        let info = MediaInfo {
            file_path: PathBuf::from("/test/audio.mkv"),
            file_size: 1024,
            container: Container::Mkv,
            duration: None,
            video_tracks: vec![],
            audio_tracks: vec![],
            subtitle_tracks: vec![],
        };
        assert!(!Condition::Codec(vec![VideoCodec::H265]).evaluate(&info));
        assert!(!Condition::HdrFormat(vec![HdrFormat::Sdr]).evaluate(&info));
        assert!(!Condition::MinResolution {
            width: 0,
            height: 0
        }
        .evaluate(&info));
        assert!(!Condition::MinBitDepth(8).evaluate(&info));
    }

    #[test]
    fn no_dv_info_returns_false() {
        let mut info = make_test_info();
        info.video_tracks[0].dolby_vision = None;
        assert!(!Condition::DolbyVisionProfile(vec![7]).evaluate(&info));
    }

    #[test]
    fn no_bit_depth_returns_false() {
        let mut info = make_test_info();
        info.video_tracks[0].bit_depth = None;
        assert!(!Condition::MinBitDepth(8).evaluate(&info));
    }
}
