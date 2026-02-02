//! MP4 atom definitions and parsing.

use super::SampleTable;

/// Four-character atom type code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtomType(pub [u8; 4]);

impl AtomType {
    pub const FTYP: Self = Self(*b"ftyp");
    pub const MOOV: Self = Self(*b"moov");
    pub const MDAT: Self = Self(*b"mdat");
    pub const MVHD: Self = Self(*b"mvhd");
    pub const TRAK: Self = Self(*b"trak");
    pub const TKHD: Self = Self(*b"tkhd");
    pub const MDIA: Self = Self(*b"mdia");
    pub const MDHD: Self = Self(*b"mdhd");
    pub const HDLR: Self = Self(*b"hdlr");
    pub const MINF: Self = Self(*b"minf");
    pub const STBL: Self = Self(*b"stbl");
    pub const STSD: Self = Self(*b"stsd");
    pub const STTS: Self = Self(*b"stts");
    pub const STSS: Self = Self(*b"stss");
    pub const STSC: Self = Self(*b"stsc");
    pub const STSZ: Self = Self(*b"stsz");
    pub const STCO: Self = Self(*b"stco");
    pub const CO64: Self = Self(*b"co64");
    pub const CTTS: Self = Self(*b"ctts");
    pub const FREE: Self = Self(*b"free");
    pub const SKIP: Self = Self(*b"skip");
    pub const UDTA: Self = Self(*b"udta");

    /// Create from bytes.
    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        Self(bytes)
    }

    /// Get the 4-char code as a string.
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("????")
    }
}

impl std::fmt::Display for AtomType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Parsed atom header.
#[derive(Debug, Clone)]
pub struct Atom {
    /// Atom type code.
    pub atom_type: AtomType,
    /// Atom size including header.
    pub size: u64,
    /// File offset where atom data starts (after header).
    pub data_offset: u64,
    /// Size of the header (8 or 16 bytes).
    pub header_size: u8,
}

impl Atom {
    /// Get the data size (size - header).
    pub fn data_size(&self) -> u64 {
        self.size.saturating_sub(self.header_size as u64)
    }

    /// Check if this atom contains child atoms.
    pub fn is_container(&self) -> bool {
        matches!(
            self.atom_type,
            AtomType::MOOV
                | AtomType::TRAK
                | AtomType::MDIA
                | AtomType::MINF
                | AtomType::STBL
                | AtomType::UDTA
        )
    }
}

/// Handler type for a track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerType {
    Video,
    Audio,
    Hint,
    Meta,
    Text,
    Unknown([u8; 4]),
}

impl HandlerType {
    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        match &bytes {
            b"vide" => Self::Video,
            b"soun" => Self::Audio,
            b"hint" => Self::Hint,
            b"meta" => Self::Meta,
            b"text" => Self::Text,
            _ => Self::Unknown(bytes),
        }
    }

    pub fn is_video(&self) -> bool {
        matches!(self, Self::Video)
    }

    pub fn is_audio(&self) -> bool {
        matches!(self, Self::Audio)
    }
}

/// Track information extracted from trak atom.
#[derive(Debug, Clone)]
pub struct TrackInfo {
    /// Track ID.
    pub track_id: u32,
    /// Handler type (video/audio/etc).
    pub handler_type: HandlerType,
    /// Track duration in media timescale.
    pub duration: u64,
    /// Media timescale (samples per second for this track).
    pub timescale: u32,
    /// Sample table with all sample info.
    pub sample_table: SampleTable,
    /// Codec configuration data (avcC, hvcC, esds, etc).
    pub codec_data: Option<Vec<u8>>,
    /// Width (for video tracks).
    pub width: Option<u32>,
    /// Height (for video tracks).
    pub height: Option<u32>,
    /// Sample rate (for audio tracks).
    pub sample_rate: Option<u32>,
    /// Channel count (for audio tracks).
    pub channels: Option<u16>,
}

impl TrackInfo {
    /// Create empty track info.
    pub fn new(track_id: u32) -> Self {
        Self {
            track_id,
            handler_type: HandlerType::Unknown([0; 4]),
            duration: 0,
            timescale: 1,
            sample_table: SampleTable::default(),
            codec_data: None,
            width: None,
            height: None,
            sample_rate: None,
            channels: None,
        }
    }

    /// Get duration in seconds.
    pub fn duration_secs(&self) -> f64 {
        if self.timescale == 0 {
            0.0
        } else {
            self.duration as f64 / self.timescale as f64
        }
    }
}
