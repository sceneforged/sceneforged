//! Container format detection and parsing

pub mod mkv;
pub mod mp4;

use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::Path;

use crate::error::VideoProbeError;

/// Supported container formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    /// Matroska (.mkv, .webm)
    Matroska,
    /// MPEG-4 Part 14 (.mp4, .m4v, .mov)
    Mp4,
}

impl std::fmt::Display for Container {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Container::Matroska => write!(f, "Matroska"),
            Container::Mp4 => write!(f, "MP4"),
        }
    }
}

/// Detect container format from file magic bytes
pub fn detect_container(path: &Path) -> Result<Container, VideoProbeError> {
    let file = File::open(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            VideoProbeError::FileNotFound(path.to_path_buf())
        } else {
            VideoProbeError::Io(e)
        }
    })?;

    let mut reader = BufReader::new(file);
    detect_container_from_reader(&mut reader)
}

/// Detect container format from a reader
pub fn detect_container_from_reader<R: Read + Seek>(
    reader: &mut R,
) -> Result<Container, VideoProbeError> {
    let mut magic = [0u8; 12];
    reader.read_exact(&mut magic)?;

    // Reset reader position
    reader.rewind()?;

    // Check for Matroska/WebM (EBML header)
    // EBML starts with 0x1A 0x45 0xDF 0xA3
    if magic[0..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        return Ok(Container::Matroska);
    }

    // Check for MP4/MOV
    // ftyp box starts at offset 4 with 'ftyp'
    if &magic[4..8] == b"ftyp" {
        return Ok(Container::Mp4);
    }

    // Check for MP4 with size at beginning
    // Some MP4 files have the moov box first
    if &magic[4..8] == b"moov" || &magic[4..8] == b"mdat" || &magic[4..8] == b"free" {
        return Ok(Container::Mp4);
    }

    // Try extension-based detection as fallback
    Err(VideoProbeError::UnsupportedContainer(
        "Unable to detect container format from magic bytes".to_string(),
    ))
}

/// Get container type from file extension (fallback)
pub fn container_from_extension(path: &Path) -> Option<Container> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "mkv" | "webm" | "mka" | "mk3d" => Some(Container::Matroska),
        "mp4" | "m4v" | "m4a" | "mov" | "3gp" | "3g2" => Some(Container::Mp4),
        _ => None,
    }
}
