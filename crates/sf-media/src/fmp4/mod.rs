//! Fragmented MP4 (fMP4) serialization.
//!
//! This module generates ISO BMFF (fragmented MP4) structures for HLS serving:
//! - Init segment (ftyp + moov with track configuration)
//! - Media segments (moof + mdat with sample data)

pub(crate) mod boxes;
mod writer;

pub use boxes::Codec;
pub use writer::{
    write_init_segment, write_init_segment_multi, write_media_segment, SampleInfo, TrackConfig,
};
