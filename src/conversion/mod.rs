//! Media conversion module.
//!
//! This module handles transcoding source files to Profile B universal format
//! for HLS streaming. It includes:
//!
//! - Job queue management
//! - FFmpeg-based transcoding with hardware acceleration support
//! - Profile B compliance verification
//! - Conversion management and batch operations
//!
//! # Profile B Specification
//!
//! Profile B (universal) files must meet these requirements:
//! - Container: MP4 with faststart (moov before mdat)
//! - Video: H.264 High profile, ≤1920x1080
//! - Audio: AAC stereo
//! - Keyframes: Every 2 seconds for HLS segment alignment

mod executor;
mod manager;

pub use executor::{adaptive_crf, adaptive_crf_from_resolution, ConversionExecutor, ProfileBSettings};
pub use manager::{ConversionManager, ConversionOptions};
