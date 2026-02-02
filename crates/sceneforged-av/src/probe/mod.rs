//! Media file probing module.
//!
//! This module provides functionality for extracting metadata from media files
//! using various backends:
//!
//! - **CLI-based** (default): Uses ffprobe/mediainfo command-line tools
//! - **Native FFmpeg** (optional): Direct FFmpeg bindings via `native-ffmpeg` feature
//! - **Pure Rust** (optional): No external tools via `pure-rust-probe` feature

mod ffprobe;
mod mediainfo;
mod types;

#[cfg(feature = "native-ffmpeg")]
mod native_ffmpeg;

#[cfg(feature = "pure-rust-probe")]
mod pure_rust;

pub use ffprobe::probe_with_ffprobe;
pub use mediainfo::probe_with_mediainfo;
pub use types::*;

#[cfg(feature = "native-ffmpeg")]
pub use native_ffmpeg::probe_with_native_ffmpeg;

#[cfg(feature = "pure-rust-probe")]
pub use pure_rust::probe_with_pure_rust;

use crate::{ProbeBackend, Result};
use std::path::Path;

/// Probe a media file using the best available backend.
///
/// With `native-ffmpeg` feature: Uses native FFmpeg bindings (fastest, no subprocess)
/// Without: Prefers mediainfo CLI (better DV/HDR detection), falls back to ffprobe CLI
pub fn probe(path: &Path) -> Result<MediaInfo> {
    probe_with(path, ProbeBackend::Auto)
}

/// Probe a media file using a specific backend.
pub fn probe_with(path: &Path, backend: ProbeBackend) -> Result<MediaInfo> {
    match backend {
        ProbeBackend::Auto => {
            // Priority: pure-rust > native-ffmpeg > mediainfo > ffprobe

            // With pure-rust-probe feature, try pure Rust first (no external tools needed)
            #[cfg(feature = "pure-rust-probe")]
            {
                match pure_rust::probe_with_pure_rust(path) {
                    Ok(info) => return Ok(info),
                    Err(_) => {
                        // Fall back to other backends
                    }
                }
            }

            // With native-ffmpeg feature, try native bindings
            #[cfg(feature = "native-ffmpeg")]
            {
                match native_ffmpeg::probe_with_native_ffmpeg(path) {
                    Ok(info) => return Ok(info),
                    Err(_) => {
                        // Fall back to CLI-based probing
                    }
                }
            }

            // Try mediainfo first (better DV/HDR detection)
            match probe_with_mediainfo(path) {
                Ok(info) => Ok(info),
                Err(_) => {
                    // Fall back to ffprobe
                    probe_with_ffprobe(path)
                }
            }
        }
        ProbeBackend::Ffprobe => probe_with_ffprobe(path),
        ProbeBackend::MediaInfo => probe_with_mediainfo(path),
        #[cfg(feature = "native-ffmpeg")]
        ProbeBackend::NativeFfmpeg => native_ffmpeg::probe_with_native_ffmpeg(path),
        #[cfg(feature = "pure-rust-probe")]
        ProbeBackend::PureRust => pure_rust::probe_with_pure_rust(path),
    }
}
