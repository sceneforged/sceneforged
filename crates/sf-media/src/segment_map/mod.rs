//! Pre-computed segment boundaries and zero-copy HLS preparation.
//!
//! - `boundary` — Keyframe-aligned segment boundary computation.
//! - `types` — Types for precomputed segment data (PreparedMedia, etc.).
//! - `builder` — Build PreparedMedia from parsed MP4 metadata.

mod boundary;
pub mod builder;
pub mod types;

pub use boundary::{compute_segment_map, KeyframeInfo, SegmentBoundary, SegmentMap};
pub use builder::build_prepared_media;
pub use types::{DataRange, PrecomputedSegment, PreparedMedia};
