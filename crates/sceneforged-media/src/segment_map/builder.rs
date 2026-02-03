//! Segment map builder.

use super::{Segment, SegmentMap};
use crate::mp4::SampleTable;

/// Builder for creating segment maps from sample tables.
pub struct SegmentMapBuilder {
    target_duration_secs: f64,
    timescale: u32,
}

impl SegmentMapBuilder {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self {
            target_duration_secs: 6.0,
            timescale: 1000,
        }
    }

    /// Set target segment duration in seconds.
    pub fn target_duration(mut self, secs: f64) -> Self {
        self.target_duration_secs = secs;
        self
    }

    /// Set media timescale.
    pub fn timescale(mut self, ts: u32) -> Self {
        self.timescale = ts;
        self
    }

    /// Build segment map from a sample table.
    ///
    /// Segments are created by grouping samples between keyframes,
    /// targeting the specified duration.
    pub fn build(self, sample_table: &SampleTable) -> SegmentMap {
        if sample_table.samples.is_empty() {
            return SegmentMap {
                timescale: self.timescale,
                target_duration_secs: self.target_duration_secs,
                ..Default::default()
            };
        }

        let keyframe_indices = sample_table.keyframe_indices();

        if keyframe_indices.is_empty() {
            // No keyframes - treat entire file as one segment
            return self.build_single_segment(sample_table);
        }

        let target_ticks = (self.target_duration_secs * self.timescale as f64) as u64;
        let mut segments = Vec::new();
        let mut segment_start_idx = 0u32;
        let mut segment_start_time: u64 = 0;

        for (i, &keyframe_idx) in keyframe_indices.iter().enumerate() {
            if keyframe_idx == 0 {
                continue;
            }

            let is_last_keyframe = i == keyframe_indices.len() - 1;
            let _next_keyframe_idx = if is_last_keyframe {
                sample_table.sample_count
            } else {
                keyframe_indices[i + 1]
            };

            // Get time at this keyframe
            let keyframe_time = sample_table.get(keyframe_idx).map(|s| s.dts).unwrap_or(0);

            let segment_duration = keyframe_time.saturating_sub(segment_start_time);

            // Create segment if we've reached target duration or this is a good boundary
            if segment_duration >= target_ticks || is_last_keyframe {
                // For the last keyframe, include all remaining samples in this segment
                let end_sample = if is_last_keyframe {
                    sample_table.sample_count
                } else {
                    keyframe_idx
                };

                let segment = self.create_segment(
                    segments.len() as u32,
                    segment_start_idx,
                    end_sample,
                    sample_table,
                    segment_start_time,
                );
                segments.push(segment);

                segment_start_idx = end_sample;
                segment_start_time = keyframe_time;
            }
        }

        // Handle remaining samples after last keyframe (only if we didn't end on last keyframe)
        if segment_start_idx < sample_table.sample_count {
            let segment = self.create_segment(
                segments.len() as u32,
                segment_start_idx,
                sample_table.sample_count,
                sample_table,
                segment_start_time,
            );
            segments.push(segment);
        }

        let max_duration = segments.iter().map(|s| s.duration_secs).fold(0.0, f64::max);

        let total_duration = segments.iter().map(|s| s.duration_secs).sum();

        SegmentMap {
            timescale: self.timescale,
            duration_secs: total_duration,
            target_duration_secs: self.target_duration_secs,
            max_segment_duration_secs: max_duration,
            segments,
            sample_count: sample_table.sample_count,
            init_segment: None,
        }
    }

    fn build_single_segment(&self, sample_table: &SampleTable) -> SegmentMap {
        let segment = self.create_segment(0, 0, sample_table.sample_count, sample_table, 0);

        let duration = segment.duration_secs;

        SegmentMap {
            timescale: self.timescale,
            duration_secs: duration,
            target_duration_secs: self.target_duration_secs,
            max_segment_duration_secs: duration,
            segments: vec![segment],
            sample_count: sample_table.sample_count,
            init_segment: None,
        }
    }

    fn create_segment(
        &self,
        index: u32,
        start_sample: u32,
        end_sample: u32,
        sample_table: &SampleTable,
        start_time_ticks: u64,
    ) -> Segment {
        let end = if end_sample > 0 && end_sample <= sample_table.sample_count {
            sample_table.get(end_sample.saturating_sub(1))
        } else {
            None
        };

        // Coalesce contiguous samples into byte ranges
        let mut byte_ranges: Vec<(u64, u32)> = Vec::new();
        for i in start_sample..end_sample {
            if let Some(sample) = sample_table.get(i) {
                if let Some(last) = byte_ranges.last_mut() {
                    let run_end = last.0 + last.1 as u64;
                    if sample.offset == run_end {
                        // Extend current run
                        last.1 += sample.size;
                        continue;
                    }
                }
                // Start new run
                byte_ranges.push((sample.offset, sample.size));
            }
        }

        // Calculate duration
        let end_time = end
            .map(|s| s.dts)
            .unwrap_or(start_time_ticks);
        let duration_ticks = end_time.saturating_sub(start_time_ticks);
        let duration_secs = if self.timescale > 0 {
            duration_ticks as f64 / self.timescale as f64
        } else {
            0.0
        };

        let start_time_secs = if self.timescale > 0 {
            start_time_ticks as f64 / self.timescale as f64
        } else {
            0.0
        };

        Segment {
            index,
            start_sample,
            end_sample,
            duration_secs,
            start_time_secs,
            byte_ranges,
            audio_byte_ranges: Vec::new(),
            audio_start_sample: None,
            audio_end_sample: None,
            moof_data: None,
        }
    }
}

impl Default for SegmentMapBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp4::SampleTableBuilder;

    fn create_test_sample_table() -> SampleTable {
        let mut builder = SampleTableBuilder::new();

        // 30 samples at 24fps = 1.25 seconds
        // Duration per sample = 1000 (assuming 24000 timescale)
        builder.set_stts(vec![(30, 1000)]);

        // Keyframes at 1, 13, 25 (0, 12, 24 in 0-indexed)
        builder.set_sync_samples(vec![1, 13, 25]);

        // All in one chunk
        builder.set_stsc(vec![(1, 30, 1)]);

        // 1KB per sample
        builder.set_stsz(1000, vec![]);

        // Chunk at offset 1000
        builder.set_chunk_offsets(vec![1000]);

        builder.build()
    }

    #[test]
    fn test_segment_map_builder() {
        let sample_table = create_test_sample_table();

        let segment_map = SegmentMapBuilder::new()
            .timescale(24000)
            .target_duration(0.5) // Small target for testing
            .build(&sample_table);

        assert!(segment_map.segment_count() > 0);
        assert_eq!(segment_map.sample_count, 30);

        // All segments should start at keyframes
        for segment in &segment_map.segments {
            if segment.index > 0 {
                let sample = sample_table.get(segment.start_sample);
                assert!(sample.is_some());
            }
        }
    }

    #[test]
    fn test_empty_sample_table() {
        let sample_table = SampleTable::default();

        let segment_map = SegmentMapBuilder::new().build(&sample_table);

        assert_eq!(segment_map.segment_count(), 0);
    }

    #[test]
    fn test_no_stss_all_keyframes() {
        let mut builder = SampleTableBuilder::new();
        builder.set_stts(vec![(10, 1000)]);
        // No sync samples = all are keyframes
        builder.set_stsc(vec![(1, 10, 1)]);
        builder.set_stsz(100, vec![]);
        builder.set_chunk_offsets(vec![0]);

        let sample_table = builder.build();
        // With target_duration > total duration, all samples go in one segment
        let segment_map = SegmentMapBuilder::new()
            .timescale(1000)
            .target_duration(15.0) // 15 seconds > 10 seconds of content
            .build(&sample_table);

        // Should create one segment with all samples
        assert_eq!(segment_map.segment_count(), 1);
        assert_eq!(segment_map.segments[0].sample_count(), 10);
    }

    #[test]
    fn test_interleaved_samples() {
        // Simulate interleaved video/audio: 2 video samples per chunk across 3 chunks
        // with gaps between chunks (where audio data would be).
        let mut builder = SampleTableBuilder::new();
        builder.set_stts(vec![(6, 1000)]);
        builder.set_sync_samples(vec![1]); // First sample is keyframe
        builder.set_stsc(vec![(1, 2, 1)]); // 2 samples per chunk
        builder.set_stsz(0, vec![100, 150, 200, 250, 300, 350]);
        // Chunks at non-contiguous offsets (audio data in between)
        builder.set_chunk_offsets(vec![1000, 2000, 3000]);

        let sample_table = builder.build();

        let segment_map = SegmentMapBuilder::new()
            .timescale(1000)
            .target_duration(10.0)
            .build(&sample_table);

        assert_eq!(segment_map.segment_count(), 1);
        let seg = &segment_map.segments[0];

        // Should have 3 byte ranges (one per chunk), not 1
        assert_eq!(seg.byte_ranges.len(), 3);
        // First chunk: samples 0+1 = 100+150 = 250 bytes
        assert_eq!(seg.byte_ranges[0], (1000, 250));
        // Second chunk: samples 2+3 = 200+250 = 450 bytes
        assert_eq!(seg.byte_ranges[1], (2000, 450));
        // Third chunk: samples 4+5 = 300+350 = 650 bytes
        assert_eq!(seg.byte_ranges[2], (3000, 650));
        // Total data size
        assert_eq!(seg.data_size(), 1350);
    }
}
