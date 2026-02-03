//! HLS playlist types.

use serde::{Deserialize, Serialize};

/// A stream variant in a master playlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variant {
    /// Peak bandwidth in bits per second.
    pub bandwidth: u64,
    /// Optional resolution as (width, height).
    pub resolution: Option<(u32, u32)>,
    /// Codec string (e.g. "avc1.64001f,mp4a.40.2").
    pub codecs: String,
    /// URI to the media playlist for this variant.
    pub uri: String,
}

/// A single segment in a media playlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    /// Segment duration in seconds.
    pub duration: f64,
    /// URI for this segment.
    pub uri: String,
    /// Optional human-readable title.
    pub title: Option<String>,
}

/// An HLS master playlist containing multiple stream variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterPlaylist {
    /// Stream variants ordered by bandwidth.
    pub variants: Vec<Variant>,
}

/// An HLS media playlist describing a sequence of segments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaPlaylist {
    /// Maximum segment duration in integer seconds (rounded up).
    pub target_duration: u32,
    /// Sequence number of the first segment.
    pub media_sequence: u64,
    /// Ordered list of segments.
    pub segments: Vec<Segment>,
    /// Whether the playlist is complete (VOD). If true, `#EXT-X-ENDLIST` is emitted.
    pub ended: bool,
    /// Optional URI for the initialization segment (`#EXT-X-MAP`).
    pub init_segment_uri: Option<String>,
}
