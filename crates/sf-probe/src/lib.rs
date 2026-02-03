//! # sf-probe
//!
//! Pure-Rust media file probing with HDR and Dolby Vision detection.
//!
//! This crate extracts metadata from video files (MKV, MP4, M4V) without
//! requiring external tools like `ffprobe` or `mediainfo`. It detects:
//!
//! - Container format (Matroska, MP4)
//! - Video tracks with codec, resolution, frame rate, and HDR format
//! - Audio tracks with codec, channel count, and sample rate
//! - Subtitle tracks
//! - HDR format (SDR, HDR10, HDR10+, HLG, Dolby Vision)
//!
//! ## Quick start
//!
//! ```no_run
//! use sf_probe::{RustProber, Prober};
//! use std::path::Path;
//!
//! let prober = RustProber::new();
//! let info = prober.probe(Path::new("movie.mkv")).unwrap();
//! println!("Container: {}", info.container);
//! if let Some(v) = info.primary_video() {
//!     println!("Video: {} {}x{} {:?}", v.codec, v.width, v.height, v.hdr_format);
//! }
//! ```

pub mod composite;
mod hdr;
pub mod prober;
pub mod rust_prober;
pub mod types;

// Re-export key types at crate root for convenience.
pub use composite::CompositeProber;
pub use prober::Prober;
pub use rust_prober::RustProber;
pub use types::{AudioTrack, DvInfo, MediaInfo, SubtitleTrack, VideoTrack};
