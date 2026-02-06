//! Types for precomputed segment map data.

use std::path::PathBuf;

/// A byte range within the source MP4 file.
#[derive(Debug, Clone)]
pub struct DataRange {
    pub file_offset: u64,
    pub length: u64,
}

/// A precomputed HLS segment: moof header + mdat header are in memory,
/// sample data is read from the source file on demand.
#[derive(Debug, Clone)]
pub struct PrecomputedSegment {
    pub index: u32,
    pub start_time_secs: f64,
    pub duration_secs: f64,
    /// Pre-built moof box bytes (video traf + audio traf).
    pub moof_bytes: Vec<u8>,
    /// Pre-built mdat header bytes.
    pub mdat_header: Vec<u8>,
    /// Video byte ranges to read from the source MP4 (written to mdat first).
    pub video_data_ranges: Vec<DataRange>,
    /// Audio byte ranges to read from the source MP4 (written to mdat after video).
    pub audio_data_ranges: Vec<DataRange>,
    /// Total length of all data ranges (= mdat payload size).
    pub data_length: u64,
}

/// Fully prepared media file for zero-copy HLS serving.
#[derive(Debug, Clone)]
pub struct PreparedMedia {
    /// Path to the source MP4 file.
    pub file_path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub duration_secs: f64,
    /// ftyp + moov init segment (served as init.mp4).
    pub init_segment: Vec<u8>,
    /// HLS media playlist string (served as index.m3u8).
    pub variant_playlist: String,
    /// Precomputed segments.
    pub segments: Vec<PrecomputedSegment>,
    /// Target segment duration (for EXT-X-TARGETDURATION).
    pub target_duration: u32,
}
