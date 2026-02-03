//! sf-media: fragmented MP4 serialization, HLS playlist generation, and segment mapping.
//!
//! This crate provides the media container and streaming infrastructure for
//! sceneforged, enabling HLS delivery of video content.
//!
//! # Modules
//!
//! - [`fmp4`] - Fragmented MP4 (ISO BMFF) serialization: init segments and media segments
//! - [`hls`] - HLS playlist generation: master and media playlists (M3U8)
//! - [`segment_map`] - Pre-computed segment boundaries aligned to keyframes

pub mod fmp4;
pub mod hls;
pub mod segment_map;

// Re-export commonly used items at the crate root.
pub use fmp4::{write_init_segment, write_media_segment, Codec, SampleInfo, TrackConfig};
pub use hls::{
    generate_master_playlist, generate_media_playlist, MasterPlaylist, MediaPlaylist, Segment,
    Variant,
};
pub use segment_map::{
    compute_segment_map, KeyframeInfo, SegmentBoundary, SegmentMap,
};
