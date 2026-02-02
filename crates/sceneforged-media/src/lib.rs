//! Sceneforged-Media: MP4/MKV parsing, fMP4 serialization, and HLS segment maps
//!
//! This crate provides media container parsing and HLS serving infrastructure
//! for sceneforged. It enables zero-copy streaming by precomputing segment maps
//! from source files.
//!
//! # Modules
//!
//! - `mp4` - MP4 container parsing (moov, sample tables, avcC/esds)
//! - `segment_map` - Precomputed HLS segment boundaries and data ranges
//! - `fmp4` - Fragmented MP4 serialization (init segment, moof/mdat)
//! - `hls` - HLS playlist generation (m3u8)
//!
//! # Architecture
//!
//! The player serves Profile B files (H.264 MP4 with faststart) via HLS without
//! re-encoding. At scan time, the segment map is built by:
//!
//! 1. Parsing the MP4 moov atom to extract sample tables
//! 2. Resolving sample offsets and sizes from stts/stsz/stss/stco
//! 3. Computing segment boundaries aligned to keyframes (~6s target)
//! 4. Pre-serializing moof boxes for each segment
//!
//! At serve time, each HLS segment is assembled from:
//! - Pre-built moof bytes (from RAM)
//! - 8-byte mdat header
//! - Raw sample data (via sendfile zero-copy from source file)

pub mod error;
pub mod fmp4;
pub mod hls;
pub mod mp4;
pub mod segment_map;

pub use error::{Error, Result};
pub use fmp4::InitSegment;
pub use hls::{HlsPlaylist, MediaPlaylist};
pub use mp4::Mp4File;
pub use segment_map::SegmentMap;
