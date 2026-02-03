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
#[cfg(feature = "native-ffmpeg")]
use std::cmp::min;
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
                    Ok(mut info) => {
                        // If native FFmpeg missed HDR/DV, supplement with mediainfo
                        // (e.g. MKV files where DOVI config isn't in coded_side_data)
                        let has_hdr = info.video_tracks.iter().any(|v| {
                            v.dolby_vision.is_some()
                                || matches!(
                                    v.hdr_format,
                                    Some(
                                        HdrFormat::Hdr10
                                            | HdrFormat::Hdr10Plus
                                            | HdrFormat::DolbyVision
                                            | HdrFormat::Hlg
                                    )
                                )
                        });
                        if !has_hdr {
                            if let Ok(mi) = probe_with_mediainfo(path) {
                                merge_hdr_info(&mut info, &mi);
                            }
                        }
                        return Ok(info);
                    }
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

/// Merge HDR/DV info from a mediainfo result into an existing probe result.
///
/// Copies `hdr_format` and `dolby_vision` fields from `supplement` video tracks
/// into the corresponding tracks in `target`, matched by index position.
#[cfg(feature = "native-ffmpeg")]
fn merge_hdr_info(target: &mut MediaInfo, supplement: &MediaInfo) {
    let len = min(target.video_tracks.len(), supplement.video_tracks.len());
    for i in 0..len {
        let src = &supplement.video_tracks[i];
        let dst = &mut target.video_tracks[i];

        if dst.hdr_format.is_none() && src.hdr_format.is_some() {
            dst.hdr_format = src.hdr_format;
        }
        if dst.dolby_vision.is_none() && src.dolby_vision.is_some() {
            dst.dolby_vision = src.dolby_vision.clone();
        }
    }
}
