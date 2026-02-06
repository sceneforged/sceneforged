//! sf-media: fragmented MP4 serialization, HLS playlist generation, segment
//! mapping, and MP4 moov parsing.
//!
//! This crate provides the media container and streaming infrastructure for
//! sceneforged, enabling HLS delivery of video content.
//!
//! # Modules
//!
//! - [`fmp4`] - Fragmented MP4 (ISO BMFF) serialization: init segments and media segments
//! - [`hls`] - HLS playlist generation: master and media playlists (M3U8)
//! - [`mp4`] - MP4 moov atom parser: extract sample tables and codec config
//! - [`segment_map`] - Pre-computed segment boundaries and zero-copy HLS preparation

pub mod fmp4;
pub mod hls;
pub mod mp4;
pub mod segment_map;

// Re-export commonly used items at the crate root.
pub use fmp4::{
    write_init_segment, write_init_segment_multi, write_media_segment, Codec, SampleInfo,
    TrackConfig,
};
pub use hls::{
    generate_master_playlist, generate_media_playlist, MasterPlaylist, MediaPlaylist, Segment,
    Variant,
};
pub use mp4::{parse_moov, Mp4Metadata, TrackInfo};
pub use segment_map::{
    build_prepared_media, compute_segment_map, DataRange, KeyframeInfo, PrecomputedSegment,
    PreparedMedia, SegmentBoundary, SegmentMap,
};
