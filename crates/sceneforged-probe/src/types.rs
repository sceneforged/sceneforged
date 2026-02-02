//! Core types for video metadata representation

use std::fmt;

/// Complete media file information
#[derive(Debug, Clone)]
pub struct MediaInfo {
    /// Path to the probed file
    pub file_path: String,
    /// File size in bytes
    pub file_size: u64,
    /// Container format (e.g., "Matroska", "MP4")
    pub container: String,
    /// Duration in milliseconds
    pub duration_ms: Option<u64>,
    /// Video tracks in the file
    pub video_tracks: Vec<VideoTrack>,
    /// Audio tracks in the file
    pub audio_tracks: Vec<AudioTrack>,
    /// Subtitle tracks in the file
    pub subtitle_tracks: Vec<SubtitleTrack>,
}

/// Video track information
#[derive(Debug, Clone)]
pub struct VideoTrack {
    /// Track index (0-based)
    pub index: u32,
    /// Codec identifier (e.g., "HEVC", "AVC", "AV1")
    pub codec: String,
    /// Video width in pixels
    pub width: u32,
    /// Video height in pixels
    pub height: u32,
    /// Frame rate (frames per second)
    pub frame_rate: Option<f64>,
    /// Bit depth (8, 10, 12)
    pub bit_depth: Option<u8>,
    /// Color primaries (BT.709, BT.2020, etc.)
    pub color_primaries: Option<ColorPrimaries>,
    /// Transfer characteristics
    pub transfer_characteristics: Option<TransferCharacteristics>,
    /// Matrix coefficients
    pub matrix_coefficients: Option<MatrixCoefficients>,
    /// HDR format if detected
    pub hdr_format: Option<HdrFormat>,
    /// Codec-specific private data
    pub codec_private: Option<Vec<u8>>,
}

/// Audio track information
#[derive(Debug, Clone)]
pub struct AudioTrack {
    /// Track index (0-based)
    pub index: u32,
    /// Codec identifier (e.g., "AAC", "AC-3", "DTS")
    pub codec: String,
    /// Number of channels
    pub channels: u8,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Bit depth
    pub bit_depth: Option<u8>,
    /// Language code (ISO 639-2)
    pub language: Option<String>,
    /// Track title/name
    pub title: Option<String>,
    /// Whether this is the default track
    pub default: bool,
}

/// Subtitle track information
#[derive(Debug, Clone)]
pub struct SubtitleTrack {
    /// Track index (0-based)
    pub index: u32,
    /// Codec/format (e.g., "SRT", "ASS", "PGS")
    pub codec: String,
    /// Language code (ISO 639-2)
    pub language: Option<String>,
    /// Track title/name
    pub title: Option<String>,
    /// Whether this is the default track
    pub default: bool,
    /// Whether this is a forced track
    pub forced: bool,
}

/// HDR format variants
#[derive(Debug, Clone, PartialEq)]
pub enum HdrFormat {
    /// Standard Dynamic Range
    Sdr,
    /// HDR10 (SMPTE ST.2084 PQ with static metadata)
    Hdr10 {
        mastering_display: Option<MasteringDisplay>,
        content_light_level: Option<ContentLightLevel>,
    },
    /// HDR10+ (HDR10 with dynamic metadata)
    Hdr10Plus {
        mastering_display: Option<MasteringDisplay>,
        content_light_level: Option<ContentLightLevel>,
    },
    /// Hybrid Log-Gamma
    Hlg,
    /// Dolby Vision
    DolbyVision {
        /// DV profile (0-10)
        profile: u8,
        /// DV level
        level: Option<u8>,
        /// Base layer compatibility ID
        bl_compatibility_id: Option<u8>,
        /// Whether there's an RPU present
        rpu_present: bool,
        /// Whether there's an enhancement layer
        el_present: bool,
        /// Base layer HDR format (for dual-layer DV)
        bl_signal_compatibility: Option<Box<HdrFormat>>,
    },
}

impl fmt::Display for HdrFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HdrFormat::Sdr => write!(f, "SDR"),
            HdrFormat::Hdr10 { .. } => write!(f, "HDR10"),
            HdrFormat::Hdr10Plus { .. } => write!(f, "HDR10+"),
            HdrFormat::Hlg => write!(f, "HLG"),
            HdrFormat::DolbyVision { profile, level, .. } => {
                write!(f, "Dolby Vision Profile {}", profile)?;
                if let Some(lvl) = level {
                    write!(f, " Level {}", lvl)?;
                }
                Ok(())
            }
        }
    }
}

/// SMPTE ST.2086 mastering display metadata
#[derive(Debug, Clone, PartialEq)]
pub struct MasteringDisplay {
    /// Display primaries chromaticity coordinates (RGB)
    /// Each pair is [x, y] in units of 0.00002
    pub primaries: [[u16; 2]; 3],
    /// White point chromaticity [x, y] in units of 0.00002
    pub white_point: [u16; 2],
    /// Maximum luminance in units of 0.0001 cd/m²
    pub max_luminance: u32,
    /// Minimum luminance in units of 0.0001 cd/m²
    pub min_luminance: u32,
}

/// Content light level information (CLL)
#[derive(Debug, Clone, PartialEq)]
pub struct ContentLightLevel {
    /// Maximum Content Light Level (cd/m²)
    pub max_cll: u16,
    /// Maximum Frame-Average Light Level (cd/m²)
    pub max_fall: u16,
}

/// Color primaries (ITU-T H.273)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ColorPrimaries {
    /// BT.709 (sRGB)
    Bt709 = 1,
    /// Unspecified
    Unspecified = 2,
    /// BT.470M
    Bt470M = 4,
    /// BT.470BG
    Bt470Bg = 5,
    /// SMPTE 170M (NTSC)
    Smpte170M = 6,
    /// SMPTE 240M
    Smpte240M = 7,
    /// Generic film
    Film = 8,
    /// BT.2020
    Bt2020 = 9,
    /// SMPTE ST 428-1 (XYZ)
    Smpte428 = 10,
    /// SMPTE ST 431-2 (DCI-P3)
    SmpteRp431 = 11,
    /// SMPTE ST 432-1 (Display P3)
    SmpteEg432 = 12,
    /// EBU Tech 3213
    Ebu3213 = 22,
    /// Unknown value
    Unknown(u8),
}

impl From<u8> for ColorPrimaries {
    fn from(value: u8) -> Self {
        match value {
            1 => ColorPrimaries::Bt709,
            2 => ColorPrimaries::Unspecified,
            4 => ColorPrimaries::Bt470M,
            5 => ColorPrimaries::Bt470Bg,
            6 => ColorPrimaries::Smpte170M,
            7 => ColorPrimaries::Smpte240M,
            8 => ColorPrimaries::Film,
            9 => ColorPrimaries::Bt2020,
            10 => ColorPrimaries::Smpte428,
            11 => ColorPrimaries::SmpteRp431,
            12 => ColorPrimaries::SmpteEg432,
            22 => ColorPrimaries::Ebu3213,
            v => ColorPrimaries::Unknown(v),
        }
    }
}

/// Transfer characteristics (ITU-T H.273)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TransferCharacteristics {
    /// BT.709
    Bt709 = 1,
    /// Unspecified
    Unspecified = 2,
    /// BT.470M
    Bt470M = 4,
    /// BT.470BG
    Bt470Bg = 5,
    /// SMPTE 170M
    Smpte170M = 6,
    /// SMPTE 240M
    Smpte240M = 7,
    /// Linear
    Linear = 8,
    /// Logarithmic (100:1)
    Log100 = 9,
    /// Logarithmic (100*sqrt(10):1)
    Log316 = 10,
    /// IEC 61966-2-4
    Iec61966_2_4 = 11,
    /// BT.1361 extended
    Bt1361E = 12,
    /// IEC 61966-2-1 (sRGB)
    Iec61966_2_1 = 13,
    /// BT.2020 10-bit
    Bt2020_10 = 14,
    /// BT.2020 12-bit
    Bt2020_12 = 15,
    /// SMPTE ST 2084 (PQ) - HDR10
    SmpteSt2084 = 16,
    /// SMPTE ST 428-1
    SmpteSt428 = 17,
    /// ARIB STD-B67 (HLG)
    AribStdB67 = 18,
    /// Unknown value
    Unknown(u8),
}

impl From<u8> for TransferCharacteristics {
    fn from(value: u8) -> Self {
        match value {
            1 => TransferCharacteristics::Bt709,
            2 => TransferCharacteristics::Unspecified,
            4 => TransferCharacteristics::Bt470M,
            5 => TransferCharacteristics::Bt470Bg,
            6 => TransferCharacteristics::Smpte170M,
            7 => TransferCharacteristics::Smpte240M,
            8 => TransferCharacteristics::Linear,
            9 => TransferCharacteristics::Log100,
            10 => TransferCharacteristics::Log316,
            11 => TransferCharacteristics::Iec61966_2_4,
            12 => TransferCharacteristics::Bt1361E,
            13 => TransferCharacteristics::Iec61966_2_1,
            14 => TransferCharacteristics::Bt2020_10,
            15 => TransferCharacteristics::Bt2020_12,
            16 => TransferCharacteristics::SmpteSt2084,
            17 => TransferCharacteristics::SmpteSt428,
            18 => TransferCharacteristics::AribStdB67,
            v => TransferCharacteristics::Unknown(v),
        }
    }
}

impl TransferCharacteristics {
    /// Returns true if this transfer characteristic indicates HDR content
    pub fn is_hdr(&self) -> bool {
        matches!(
            self,
            TransferCharacteristics::SmpteSt2084 | TransferCharacteristics::AribStdB67
        )
    }
}

/// Matrix coefficients (ITU-T H.273)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MatrixCoefficients {
    /// Identity (RGB)
    Identity = 0,
    /// BT.709
    Bt709 = 1,
    /// Unspecified
    Unspecified = 2,
    /// FCC
    Fcc = 4,
    /// BT.470BG
    Bt470Bg = 5,
    /// SMPTE 170M
    Smpte170M = 6,
    /// SMPTE 240M
    Smpte240M = 7,
    /// YCgCo
    YCgCo = 8,
    /// BT.2020 non-constant luminance
    Bt2020Ncl = 9,
    /// BT.2020 constant luminance
    Bt2020Cl = 10,
    /// SMPTE ST 2085 (Y'D'zD'x)
    SmpteSt2085 = 11,
    /// Chromaticity-derived non-constant luminance
    ChromaNcl = 12,
    /// Chromaticity-derived constant luminance
    ChromaCl = 13,
    /// ICtCp
    ICtCp = 14,
    /// Unknown value
    Unknown(u8),
}

impl From<u8> for MatrixCoefficients {
    fn from(value: u8) -> Self {
        match value {
            0 => MatrixCoefficients::Identity,
            1 => MatrixCoefficients::Bt709,
            2 => MatrixCoefficients::Unspecified,
            4 => MatrixCoefficients::Fcc,
            5 => MatrixCoefficients::Bt470Bg,
            6 => MatrixCoefficients::Smpte170M,
            7 => MatrixCoefficients::Smpte240M,
            8 => MatrixCoefficients::YCgCo,
            9 => MatrixCoefficients::Bt2020Ncl,
            10 => MatrixCoefficients::Bt2020Cl,
            11 => MatrixCoefficients::SmpteSt2085,
            12 => MatrixCoefficients::ChromaNcl,
            13 => MatrixCoefficients::ChromaCl,
            14 => MatrixCoefficients::ICtCp,
            v => MatrixCoefficients::Unknown(v),
        }
    }
}
