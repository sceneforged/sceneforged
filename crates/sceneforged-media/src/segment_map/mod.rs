//! Segment map for HLS serving.
//!
//! A segment map precomputes HLS segment boundaries from an MP4 file's sample table.
//! This allows zero-copy serving of segments without re-encoding.

mod builder;

pub use builder::SegmentMapBuilder;

use crate::mp4::SampleEntry;

/// A single HLS segment definition.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct Segment {
    /// Segment index (0-based).
    pub index: u32,
    /// Start sample index in the source file.
    pub start_sample: u32,
    /// End sample index (exclusive).
    pub end_sample: u32,
    /// Duration in seconds.
    pub duration_secs: f64,
    /// Start time in seconds (from beginning of file).
    pub start_time_secs: f64,
    /// Byte ranges in the source file: `(offset, length)`.
    /// Contiguous samples are coalesced into single ranges.
    /// Interleaved files (e.g. FFmpeg output) will have multiple ranges.
    pub byte_ranges: Vec<(u64, u32)>,
    /// Pre-built moof box for this segment (if available).
    pub moof_data: Option<Vec<u8>>,
}

impl Segment {
    /// Get sample count in this segment.
    pub fn sample_count(&self) -> u32 {
        self.end_sample - self.start_sample
    }

    /// Total size of sample data in this segment.
    pub fn data_size(&self) -> u64 {
        self.byte_ranges.iter().map(|(_, len)| *len as u64).sum()
    }
}

/// Precomputed segment map for HLS serving.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct SegmentMap {
    /// Media timescale (samples per second).
    pub timescale: u32,
    /// Total duration in seconds.
    pub duration_secs: f64,
    /// Target segment duration in seconds.
    pub target_duration_secs: f64,
    /// Actual maximum segment duration.
    pub max_segment_duration_secs: f64,
    /// All segments.
    pub segments: Vec<Segment>,
    /// Total sample count.
    pub sample_count: u32,
    /// Initialization segment data (ftyp + moov for fMP4).
    pub init_segment: Option<Vec<u8>>,
}

impl SegmentMap {
    /// Create a new segment map builder.
    pub fn builder() -> SegmentMapBuilder {
        SegmentMapBuilder::new()
    }

    /// Get segment count.
    pub fn segment_count(&self) -> u32 {
        self.segments.len() as u32
    }

    /// Get a segment by index.
    pub fn get_segment(&self, index: u32) -> Option<&Segment> {
        self.segments.get(index as usize)
    }

    /// Find segment containing the given time in seconds.
    pub fn find_segment_at_time(&self, time_secs: f64) -> Option<u32> {
        for (i, seg) in self.segments.iter().enumerate() {
            if time_secs >= seg.start_time_secs
                && time_secs < seg.start_time_secs + seg.duration_secs
            {
                return Some(i as u32);
            }
        }
        None
    }

    /// Generate sample entries for a segment.
    /// Used when serving a segment to know which samples to include.
    pub fn segment_samples<'a>(
        &self,
        segment_index: u32,
        all_samples: &'a [SampleEntry],
    ) -> &'a [SampleEntry] {
        if let Some(segment) = self.get_segment(segment_index) {
            let start = segment.start_sample as usize;
            let end = (segment.end_sample as usize).min(all_samples.len());
            &all_samples[start..end]
        } else {
            &[]
        }
    }
}

impl Default for SegmentMap {
    fn default() -> Self {
        Self {
            timescale: 1000,
            duration_secs: 0.0,
            target_duration_secs: 6.0,
            max_segment_duration_secs: 0.0,
            segments: Vec::new(),
            sample_count: 0,
            init_segment: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segment_map_default() {
        let map = SegmentMap::default();
        assert_eq!(map.segment_count(), 0);
        assert_eq!(map.timescale, 1000);
    }

    #[test]
    fn test_find_segment_at_time() {
        let map = SegmentMap {
            segments: vec![
                Segment {
                    index: 0,
                    start_sample: 0,
                    end_sample: 100,
                    duration_secs: 5.0,
                    start_time_secs: 0.0,
                    byte_ranges: vec![(0, 1000)],
                    moof_data: None,
                },
                Segment {
                    index: 1,
                    start_sample: 100,
                    end_sample: 200,
                    duration_secs: 5.0,
                    start_time_secs: 5.0,
                    byte_ranges: vec![(1000, 1000)],
                    moof_data: None,
                },
            ],
            ..Default::default()
        };

        assert_eq!(map.find_segment_at_time(0.0), Some(0));
        assert_eq!(map.find_segment_at_time(2.5), Some(0));
        assert_eq!(map.find_segment_at_time(5.0), Some(1));
        assert_eq!(map.find_segment_at_time(7.5), Some(1));
        assert_eq!(map.find_segment_at_time(10.0), None);
    }
}
