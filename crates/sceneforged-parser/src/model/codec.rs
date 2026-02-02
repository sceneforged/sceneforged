//! Video and audio codec enums.

use super::ParseError;

/// Video compression standard (codec family).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum VideoStandard {
    /// H.264/AVC
    H264,
    /// H.265/HEVC
    H265,
    /// AV1
    Av1,
    /// MPEG-2
    Mpeg2,
    /// MPEG-4 Part 2 (XviD, DivX)
    Mpeg4Part2,
    /// VP9
    Vp9,
    /// VC-1 (Windows Media Video 9)
    Vc1,
}

impl std::fmt::Display for VideoStandard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VideoStandard::H264 => write!(f, "H.264"),
            VideoStandard::H265 => write!(f, "H.265"),
            VideoStandard::Av1 => write!(f, "AV1"),
            VideoStandard::Mpeg2 => write!(f, "MPEG-2"),
            VideoStandard::Mpeg4Part2 => write!(f, "MPEG-4 Part 2"),
            VideoStandard::Vp9 => write!(f, "VP9"),
            VideoStandard::Vc1 => write!(f, "VC-1"),
        }
    }
}

impl std::str::FromStr for VideoStandard {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "h264" | "h.264" | "avc" => Ok(VideoStandard::H264),
            "h265" | "h.265" | "hevc" => Ok(VideoStandard::H265),
            "av1" => Ok(VideoStandard::Av1),
            "mpeg2" | "mpeg-2" => Ok(VideoStandard::Mpeg2),
            "mpeg4part2" | "mpeg-4 part 2" | "mpeg4" => Ok(VideoStandard::Mpeg4Part2),
            "vp9" => Ok(VideoStandard::Vp9),
            "vc-1" | "vc1" => Ok(VideoStandard::Vc1),
            _ => Err(ParseError(format!("invalid video standard: {}", s))),
        }
    }
}

/// Video encoder implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum VideoEncoder {
    /// x264 (H.264 encoder)
    X264,
    /// x265 (H.265 encoder)
    X265,
    /// NVENC H.264 (NVIDIA hardware encoder)
    Nvenc264,
    /// NVENC H.265 (NVIDIA hardware encoder)
    Nvenc265,
    /// SVT-AV1 (Scalable Video Technology for AV1)
    SvtAv1,
    /// rav1e (Rust AV1 encoder)
    Rav1e,
    /// aomenc (Alliance for Open Media AV1 encoder)
    Aom,
    /// XviD (MPEG-4 Part 2 encoder)
    Xvid,
    /// DivX (MPEG-4 Part 2 encoder)
    DivX,
}

impl VideoEncoder {
    /// Returns the video standard that this encoder produces.
    pub fn standard(&self) -> VideoStandard {
        match self {
            VideoEncoder::X264 | VideoEncoder::Nvenc264 => VideoStandard::H264,
            VideoEncoder::X265 | VideoEncoder::Nvenc265 => VideoStandard::H265,
            VideoEncoder::SvtAv1 | VideoEncoder::Rav1e | VideoEncoder::Aom => VideoStandard::Av1,
            VideoEncoder::Xvid | VideoEncoder::DivX => VideoStandard::Mpeg4Part2,
        }
    }
}

impl std::fmt::Display for VideoEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VideoEncoder::X264 => write!(f, "x264"),
            VideoEncoder::X265 => write!(f, "x265"),
            VideoEncoder::Nvenc264 => write!(f, "NVENC H.264"),
            VideoEncoder::Nvenc265 => write!(f, "NVENC H.265"),
            VideoEncoder::SvtAv1 => write!(f, "SVT-AV1"),
            VideoEncoder::Rav1e => write!(f, "rav1e"),
            VideoEncoder::Aom => write!(f, "aomenc"),
            VideoEncoder::Xvid => write!(f, "XviD"),
            VideoEncoder::DivX => write!(f, "DivX"),
        }
    }
}

impl std::str::FromStr for VideoEncoder {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "x264" => Ok(VideoEncoder::X264),
            "x265" => Ok(VideoEncoder::X265),
            "nvenc264" | "nvenc h.264" | "nvenc h264" => Ok(VideoEncoder::Nvenc264),
            "nvenc265" | "nvenc h.265" | "nvenc h265" | "nvenc hevc" => Ok(VideoEncoder::Nvenc265),
            "svtav1" | "svt-av1" => Ok(VideoEncoder::SvtAv1),
            "rav1e" => Ok(VideoEncoder::Rav1e),
            "aom" | "aomenc" => Ok(VideoEncoder::Aom),
            "xvid" => Ok(VideoEncoder::Xvid),
            "divx" => Ok(VideoEncoder::DivX),
            _ => Err(ParseError(format!("invalid video encoder: {}", s))),
        }
    }
}

/// Audio codec used in the release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AudioCodec {
    /// Dolby TrueHD with Atmos
    TrueHdAtmos,
    /// Dolby TrueHD
    TrueHd,
    /// DTS-HD Master Audio
    DtsHdMa,
    /// DTS-HD (High Resolution)
    DtsHd,
    /// DTS
    Dts,
    /// DTS:X
    DtsX,
    /// Dolby Digital Plus with Atmos
    Eac3Atmos,
    /// Dolby Digital Plus (E-AC-3)
    Eac3,
    /// Dolby Digital (AC-3)
    Ac3,
    /// Advanced Audio Coding
    Aac,
    /// Free Lossless Audio Codec
    Flac,
    /// MPEG Audio Layer 3
    Mp3,
    /// Pulse Code Modulation (uncompressed)
    Pcm,
    /// Opus
    Opus,
    /// Ogg Vorbis
    Vorbis,
}

impl std::fmt::Display for AudioCodec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioCodec::TrueHdAtmos => write!(f, "TrueHD Atmos"),
            AudioCodec::TrueHd => write!(f, "TrueHD"),
            AudioCodec::DtsHdMa => write!(f, "DTS-HD MA"),
            AudioCodec::DtsHd => write!(f, "DTS-HD"),
            AudioCodec::Dts => write!(f, "DTS"),
            AudioCodec::DtsX => write!(f, "DTS:X"),
            AudioCodec::Eac3Atmos => write!(f, "EAC3 Atmos"),
            AudioCodec::Eac3 => write!(f, "EAC3"),
            AudioCodec::Ac3 => write!(f, "AC3"),
            AudioCodec::Aac => write!(f, "AAC"),
            AudioCodec::Flac => write!(f, "FLAC"),
            AudioCodec::Mp3 => write!(f, "MP3"),
            AudioCodec::Pcm => write!(f, "PCM"),
            AudioCodec::Opus => write!(f, "Opus"),
            AudioCodec::Vorbis => write!(f, "Vorbis"),
        }
    }
}

impl std::str::FromStr for AudioCodec {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "truehd atmos" | "truehd.atmos" => Ok(AudioCodec::TrueHdAtmos),
            "truehd" => Ok(AudioCodec::TrueHd),
            "dts-hd ma" | "dts-hd.ma" | "dtshdma" | "dts-hdma" => Ok(AudioCodec::DtsHdMa),
            "dts-hd" | "dtshd" => Ok(AudioCodec::DtsHd),
            "dts" => Ok(AudioCodec::Dts),
            "dts:x" | "dtsx" => Ok(AudioCodec::DtsX),
            "eac3 atmos" | "eac3.atmos" | "ddp atmos" | "ddp.atmos" => Ok(AudioCodec::Eac3Atmos),
            "eac3" | "e-ac-3" | "ddp" | "dd+" => Ok(AudioCodec::Eac3),
            "ac3" | "ac-3" | "dd" | "dolby digital" => Ok(AudioCodec::Ac3),
            "aac" => Ok(AudioCodec::Aac),
            "flac" => Ok(AudioCodec::Flac),
            "mp3" => Ok(AudioCodec::Mp3),
            "pcm" | "lpcm" => Ok(AudioCodec::Pcm),
            "opus" => Ok(AudioCodec::Opus),
            "vorbis" | "ogg" => Ok(AudioCodec::Vorbis),
            _ => Err(ParseError(format!("invalid audio codec: {}", s))),
        }
    }
}

/// Audio channel configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AudioChannels {
    /// Mono (1.0)
    _1_0,
    /// Stereo (2.0)
    _2_0,
    /// Surround (5.1)
    _5_1,
    /// Surround (7.1)
    _7_1,
}

impl std::fmt::Display for AudioChannels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioChannels::_1_0 => write!(f, "1.0"),
            AudioChannels::_2_0 => write!(f, "2.0"),
            AudioChannels::_5_1 => write!(f, "5.1"),
            AudioChannels::_7_1 => write!(f, "7.1"),
        }
    }
}

impl std::str::FromStr for AudioChannels {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "1.0" | "mono" => Ok(AudioChannels::_1_0),
            "2.0" | "stereo" => Ok(AudioChannels::_2_0),
            "5.1" | "6ch" => Ok(AudioChannels::_5_1),
            "7.1" | "8ch" => Ok(AudioChannels::_7_1),
            _ => Err(ParseError(format!("invalid audio channels: {}", s))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_standard_display_fromstr_roundtrip() {
        let variants = [
            VideoStandard::H264,
            VideoStandard::H265,
            VideoStandard::Av1,
            VideoStandard::Mpeg2,
            VideoStandard::Mpeg4Part2,
            VideoStandard::Vp9,
            VideoStandard::Vc1,
        ];
        for variant in variants {
            let s = variant.to_string();
            let parsed: VideoStandard = s.parse().expect("should parse");
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn video_encoder_display_fromstr_roundtrip() {
        let variants = [
            VideoEncoder::X264,
            VideoEncoder::X265,
            VideoEncoder::Nvenc264,
            VideoEncoder::Nvenc265,
            VideoEncoder::SvtAv1,
            VideoEncoder::Rav1e,
            VideoEncoder::Aom,
            VideoEncoder::Xvid,
            VideoEncoder::DivX,
        ];
        for variant in variants {
            let s = variant.to_string();
            let parsed: VideoEncoder = s.parse().expect("should parse");
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn video_encoder_standard() {
        assert_eq!(VideoEncoder::X264.standard(), VideoStandard::H264);
        assert_eq!(VideoEncoder::X265.standard(), VideoStandard::H265);
        assert_eq!(VideoEncoder::Nvenc264.standard(), VideoStandard::H264);
        assert_eq!(VideoEncoder::Nvenc265.standard(), VideoStandard::H265);
        assert_eq!(VideoEncoder::SvtAv1.standard(), VideoStandard::Av1);
        assert_eq!(VideoEncoder::Rav1e.standard(), VideoStandard::Av1);
        assert_eq!(VideoEncoder::Aom.standard(), VideoStandard::Av1);
        assert_eq!(VideoEncoder::Xvid.standard(), VideoStandard::Mpeg4Part2);
        assert_eq!(VideoEncoder::DivX.standard(), VideoStandard::Mpeg4Part2);
    }

    #[test]
    fn audio_codec_display_fromstr_roundtrip() {
        let variants = [
            AudioCodec::TrueHdAtmos,
            AudioCodec::TrueHd,
            AudioCodec::DtsHdMa,
            AudioCodec::DtsHd,
            AudioCodec::Dts,
            AudioCodec::DtsX,
            AudioCodec::Eac3Atmos,
            AudioCodec::Eac3,
            AudioCodec::Ac3,
            AudioCodec::Aac,
            AudioCodec::Flac,
            AudioCodec::Mp3,
            AudioCodec::Pcm,
            AudioCodec::Opus,
            AudioCodec::Vorbis,
        ];
        for variant in variants {
            let s = variant.to_string();
            let parsed: AudioCodec = s.parse().expect("should parse");
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn audio_channels_display_fromstr_roundtrip() {
        let variants = [
            AudioChannels::_1_0,
            AudioChannels::_2_0,
            AudioChannels::_5_1,
            AudioChannels::_7_1,
        ];
        for variant in variants {
            let s = variant.to_string();
            let parsed: AudioChannels = s.parse().expect("should parse");
            assert_eq!(variant, parsed);
        }
    }
}
