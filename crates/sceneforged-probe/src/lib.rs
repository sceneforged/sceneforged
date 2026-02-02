//! # sceneforged-probe
//!
//! Pure Rust video file probing with HDR/Dolby Vision detection.
//!
//! This crate provides functionality to extract metadata from video files,
//! including HDR format detection (HDR10, HDR10+, HLG, Dolby Vision).
//!
//! ## Features
//!
//! - Container parsing: MKV/WebM (via `matroska`), MP4/MOV (via `mp4parse`)
//! - Video codec analysis: HEVC/H.265 with NAL unit parsing
//! - HDR detection: HDR10, HDR10+, HLG, Dolby Vision
//! - No external tool dependencies (pure Rust)
//!
//! ## Example
//!
//! ```no_run
//! use sceneforged_probe::{probe_file, HdrFormat};
//!
//! let info = sceneforged_probe::probe_file("movie.mkv").unwrap();
//!
//! println!("Container: {}", info.container);
//! println!("Duration: {:?}ms", info.duration_ms);
//!
//! for video in &info.video_tracks {
//!     println!("Video: {} {}x{}", video.codec, video.width, video.height);
//!
//!     if let Some(ref hdr) = video.hdr_format {
//!         match hdr {
//!             HdrFormat::DolbyVision { profile, .. } => {
//!                 println!("  Dolby Vision Profile {}", profile);
//!             }
//!             HdrFormat::Hdr10 { .. } => println!("  HDR10"),
//!             HdrFormat::Hdr10Plus { .. } => println!("  HDR10+"),
//!             HdrFormat::Hlg => println!("  HLG"),
//!             HdrFormat::Sdr => println!("  SDR"),
//!         }
//!     }
//! }
//!
//! for audio in &info.audio_tracks {
//!     println!("Audio: {} {}ch {}Hz",
//!         audio.codec, audio.channels, audio.sample_rate);
//! }
//! ```

pub mod codec;
pub mod container;
pub mod error;
pub mod hdr;
pub mod types;

pub use error::VideoProbeError;
pub use types::*;

use std::path::Path;

use container::Container;

/// Probe a media file and extract metadata
///
/// This is the main entry point for the library. It automatically detects
/// the container format and extracts video, audio, and subtitle track
/// information, including HDR format detection.
///
/// # Arguments
///
/// * `path` - Path to the media file
///
/// # Returns
///
/// Returns `MediaInfo` containing all extracted metadata, or an error
/// if the file cannot be read or parsed.
///
/// # Example
///
/// ```no_run
/// let info = sceneforged_probe::probe_file("/path/to/video.mkv").unwrap();
/// println!("Found {} video tracks", info.video_tracks.len());
/// ```
pub fn probe_file<P: AsRef<Path>>(path: P) -> Result<MediaInfo, VideoProbeError> {
    let path = path.as_ref();

    // Check file exists
    if !path.exists() {
        return Err(VideoProbeError::FileNotFound(path.to_path_buf()));
    }

    // Detect container format
    let container_type = container::detect_container(path).or_else(|_| {
        // Fall back to extension-based detection
        container::container_from_extension(path)
            .ok_or_else(|| VideoProbeError::UnsupportedContainer("Unknown".to_string()))
    })?;

    // Parse based on container type
    match container_type {
        Container::Matroska => container::mkv::probe(path),
        Container::Mp4 => container::mp4::probe(path),
    }
}

/// Probe a media file from a byte slice
///
/// This function allows probing video data that's already in memory,
/// useful for streaming or embedded scenarios.
///
/// # Arguments
///
/// * `data` - Byte slice containing the media file
/// * `container_hint` - Optional hint about the container format
///
/// # Returns
///
/// Returns `MediaInfo` containing all extracted metadata.
pub fn probe_bytes(
    data: &[u8],
    container_hint: Option<Container>,
) -> Result<MediaInfo, VideoProbeError> {
    use std::io::Cursor;

    let container_type = if let Some(hint) = container_hint {
        hint
    } else {
        let mut cursor = Cursor::new(data);
        container::detect_container_from_reader(&mut cursor)?
    };

    // For bytes, we need to create temp implementations
    // This is a simplified version - full implementation would parse from memory
    match container_type {
        Container::Matroska => {
            // Use matroska crate's ability to read from any Read + Seek
            let cursor = Cursor::new(data);
            let mkv = matroska::Matroska::open(cursor)
                .map_err(|e| VideoProbeError::ContainerParse(format!("MKV parse error: {}", e)))?;

            // Convert to MediaInfo (simplified - full impl in mkv.rs)
            let duration_ms = mkv.info.duration.map(|d| d.as_millis() as u64);

            Ok(MediaInfo {
                file_path: "<memory>".to_string(),
                file_size: data.len() as u64,
                container: "Matroska".to_string(),
                duration_ms,
                video_tracks: vec![],
                audio_tracks: vec![],
                subtitle_tracks: vec![],
            })
        }
        Container::Mp4 => {
            let mut cursor = Cursor::new(data);
            let _context = mp4parse::read_mp4(&mut cursor).map_err(|e| {
                VideoProbeError::ContainerParse(format!("MP4 parse error: {:?}", e))
            })?;

            Ok(MediaInfo {
                file_path: "<memory>".to_string(),
                file_size: data.len() as u64,
                container: "MP4".to_string(),
                duration_ms: None,
                video_tracks: vec![],
                audio_tracks: vec![],
                subtitle_tracks: vec![],
            })
        }
    }
}

/// Check if a file appears to be a supported video format
///
/// This performs a quick check of the file's magic bytes without
/// fully parsing the container.
///
/// # Arguments
///
/// * `path` - Path to the file to check
///
/// # Returns
///
/// Returns `true` if the file appears to be a supported video format.
pub fn is_supported_format<P: AsRef<Path>>(path: P) -> bool {
    container::detect_container(path.as_ref()).is_ok()
}

/// Get the detected container format for a file
///
/// # Arguments
///
/// * `path` - Path to the file to check
///
/// # Returns
///
/// Returns the detected `Container` type or an error.
pub fn detect_container<P: AsRef<Path>>(path: P) -> Result<Container, VideoProbeError> {
    container::detect_container(path.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsupported_format() {
        let result = probe_file("/nonexistent/file.xyz");
        assert!(result.is_err());
    }
}
