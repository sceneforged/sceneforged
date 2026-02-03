//! Media-domain enums for containers, codecs, HDR formats, profiles, and more.
//!
//! All enums serialize in lowercase (via `serde(rename_all = "lowercase")`) and
//! implement `Display` manually for consistent string representation.

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// Container
// ---------------------------------------------------------------------------

/// Supported container formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Container {
    Mkv,
    Mp4,
}

impl fmt::Display for Container {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mkv => write!(f, "mkv"),
            Self::Mp4 => write!(f, "mp4"),
        }
    }
}

// ---------------------------------------------------------------------------
// VideoCodec
// ---------------------------------------------------------------------------

/// Supported video codecs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VideoCodec {
    H264,
    H265,
    Av1,
    Vp9,
}

impl fmt::Display for VideoCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::H264 => write!(f, "h264"),
            Self::H265 => write!(f, "h265"),
            Self::Av1 => write!(f, "av1"),
            Self::Vp9 => write!(f, "vp9"),
        }
    }
}

// ---------------------------------------------------------------------------
// AudioCodec
// ---------------------------------------------------------------------------

/// Supported audio codecs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioCodec {
    Aac,
    Ac3,
    Eac3,
    #[serde(rename = "truehd")]
    TrueHd,
    Dts,
    #[serde(rename = "dtshd")]
    DtsHd,
    Flac,
    Opus,
}

impl fmt::Display for AudioCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Aac => write!(f, "aac"),
            Self::Ac3 => write!(f, "ac3"),
            Self::Eac3 => write!(f, "eac3"),
            Self::TrueHd => write!(f, "truehd"),
            Self::Dts => write!(f, "dts"),
            Self::DtsHd => write!(f, "dtshd"),
            Self::Flac => write!(f, "flac"),
            Self::Opus => write!(f, "opus"),
        }
    }
}

// ---------------------------------------------------------------------------
// HdrFormat
// ---------------------------------------------------------------------------

/// HDR format classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HdrFormat {
    Sdr,
    Hdr10,
    #[serde(rename = "hdr10plus")]
    Hdr10Plus,
    #[serde(rename = "dolbyvision")]
    DolbyVision,
    Hlg,
}

impl fmt::Display for HdrFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sdr => write!(f, "sdr"),
            Self::Hdr10 => write!(f, "hdr10"),
            Self::Hdr10Plus => write!(f, "hdr10plus"),
            Self::DolbyVision => write!(f, "dolbyvision"),
            Self::Hlg => write!(f, "hlg"),
        }
    }
}

// ---------------------------------------------------------------------------
// Profile
// ---------------------------------------------------------------------------

/// Media file profile classification.
///
/// - **A**: High-quality source (HDR/DV/4K).
/// - **B**: Universal playback (MP4, H.264, SDR, AAC stereo).
/// - **C**: Unsupported/pending conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Profile {
    A,
    B,
    #[default]
    C,
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::A => write!(f, "A"),
            Self::B => write!(f, "B"),
            Self::C => write!(f, "C"),
        }
    }
}

// ---------------------------------------------------------------------------
// ItemKind
// ---------------------------------------------------------------------------

/// Kind of library item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    Movie,
    Series,
    Season,
    Episode,
}

impl fmt::Display for ItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Movie => write!(f, "movie"),
            Self::Series => write!(f, "series"),
            Self::Season => write!(f, "season"),
            Self::Episode => write!(f, "episode"),
        }
    }
}

// ---------------------------------------------------------------------------
// FileRole
// ---------------------------------------------------------------------------

/// Role a media file plays for a given item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileRole {
    /// Original source file (MKV remux, etc.).
    Source,
    /// Universal playback version (Profile B MP4).
    Universal,
    /// Extra content (trailer, featurette, etc.).
    Extra,
}

impl fmt::Display for FileRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source => write!(f, "source"),
            Self::Universal => write!(f, "universal"),
            Self::Extra => write!(f, "extra"),
        }
    }
}

// ---------------------------------------------------------------------------
// StreamType
// ---------------------------------------------------------------------------

/// Type of media stream within a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamType {
    Video,
    Audio,
    Subtitle,
}

impl fmt::Display for StreamType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Video => write!(f, "video"),
            Self::Audio => write!(f, "audio"),
            Self::Subtitle => write!(f, "subtitle"),
        }
    }
}

// ---------------------------------------------------------------------------
// ImageType
// ---------------------------------------------------------------------------

/// Type of item image/artwork.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageType {
    Primary,
    Backdrop,
    Banner,
    Thumb,
    Logo,
}

impl fmt::Display for ImageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Primary => write!(f, "primary"),
            Self::Backdrop => write!(f, "backdrop"),
            Self::Banner => write!(f, "banner"),
            Self::Thumb => write!(f, "thumb"),
            Self::Logo => write!(f, "logo"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_display_and_serde() {
        assert_eq!(Container::Mkv.to_string(), "mkv");
        assert_eq!(Container::Mp4.to_string(), "mp4");
        let json = serde_json::to_string(&Container::Mkv).unwrap();
        assert_eq!(json, r#""mkv""#);
        let back: Container = serde_json::from_str(&json).unwrap();
        assert_eq!(back, Container::Mkv);
    }

    #[test]
    fn video_codec_display() {
        assert_eq!(VideoCodec::H264.to_string(), "h264");
        assert_eq!(VideoCodec::H265.to_string(), "h265");
        assert_eq!(VideoCodec::Av1.to_string(), "av1");
        assert_eq!(VideoCodec::Vp9.to_string(), "vp9");
    }

    #[test]
    fn audio_codec_serde() {
        let json = serde_json::to_string(&AudioCodec::TrueHd).unwrap();
        assert_eq!(json, r#""truehd""#);
        let back: AudioCodec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, AudioCodec::TrueHd);

        let json = serde_json::to_string(&AudioCodec::DtsHd).unwrap();
        assert_eq!(json, r#""dtshd""#);
    }

    #[test]
    fn hdr_format_display_and_serde() {
        assert_eq!(HdrFormat::Sdr.to_string(), "sdr");
        assert_eq!(HdrFormat::Hdr10Plus.to_string(), "hdr10plus");
        assert_eq!(HdrFormat::DolbyVision.to_string(), "dolbyvision");

        let json = serde_json::to_string(&HdrFormat::Hdr10Plus).unwrap();
        assert_eq!(json, r#""hdr10plus""#);
        let back: HdrFormat = serde_json::from_str(&json).unwrap();
        assert_eq!(back, HdrFormat::Hdr10Plus);
    }

    #[test]
    fn profile_default_is_c() {
        assert_eq!(Profile::default(), Profile::C);
    }

    #[test]
    fn profile_display() {
        assert_eq!(Profile::A.to_string(), "A");
        assert_eq!(Profile::B.to_string(), "B");
        assert_eq!(Profile::C.to_string(), "C");
    }

    #[test]
    fn item_kind_display() {
        assert_eq!(ItemKind::Movie.to_string(), "movie");
        assert_eq!(ItemKind::Series.to_string(), "series");
        assert_eq!(ItemKind::Season.to_string(), "season");
        assert_eq!(ItemKind::Episode.to_string(), "episode");
    }

    #[test]
    fn file_role_display() {
        assert_eq!(FileRole::Source.to_string(), "source");
        assert_eq!(FileRole::Universal.to_string(), "universal");
        assert_eq!(FileRole::Extra.to_string(), "extra");
    }

    #[test]
    fn stream_type_serde_roundtrip() {
        let st = StreamType::Subtitle;
        let json = serde_json::to_string(&st).unwrap();
        assert_eq!(json, r#""subtitle""#);
        let back: StreamType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, StreamType::Subtitle);
    }

    #[test]
    fn image_type_display() {
        assert_eq!(ImageType::Primary.to_string(), "primary");
        assert_eq!(ImageType::Backdrop.to_string(), "backdrop");
        assert_eq!(ImageType::Banner.to_string(), "banner");
        assert_eq!(ImageType::Thumb.to_string(), "thumb");
        assert_eq!(ImageType::Logo.to_string(), "logo");
    }

    #[test]
    fn enum_equality_and_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Container::Mkv);
        set.insert(Container::Mp4);
        assert!(set.contains(&Container::Mkv));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn copy_semantics() {
        let c = VideoCodec::Av1;
        let c2 = c;
        assert_eq!(c, c2);
    }
}
