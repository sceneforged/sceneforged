//! MP4 sample table parsing.
//!
//! Sample tables describe how samples (frames) are organized in the file:
//! - stts: sample durations (decoding time)
//! - stss: sync sample table (keyframes)
//! - stsc: sample-to-chunk mapping
//! - stsz: sample sizes
//! - stco/co64: chunk offsets
//! - ctts: composition time offsets (for B-frames)

/// A resolved sample entry with all information needed for serving.
#[derive(Debug, Clone, Copy)]
pub struct SampleEntry {
    /// Sample index (0-based).
    pub index: u32,
    /// File offset where sample data starts.
    pub offset: u64,
    /// Sample size in bytes.
    pub size: u32,
    /// Decode timestamp in media timescale.
    pub dts: u64,
    /// Composition time offset (for PTS calculation).
    pub cts_offset: i32,
    /// Whether this sample is a keyframe (sync sample).
    pub is_keyframe: bool,
}

impl SampleEntry {
    /// Get the presentation timestamp.
    pub fn pts(&self) -> u64 {
        (self.dts as i64 + self.cts_offset as i64).max(0) as u64
    }
}

/// Sample table containing decoded sample information.
#[derive(Debug, Clone, Default)]
pub struct SampleTable {
    /// Sample count.
    pub sample_count: u32,
    /// All resolved samples.
    pub samples: Vec<SampleEntry>,
}

impl SampleTable {
    /// Create a new sample table builder.
    pub fn builder() -> SampleTableBuilder {
        SampleTableBuilder::new()
    }

    /// Get sample by index.
    pub fn get(&self, index: u32) -> Option<&SampleEntry> {
        self.samples.get(index as usize)
    }

    /// Iterate over all samples.
    pub fn iter(&self) -> impl Iterator<Item = &SampleEntry> {
        self.samples.iter()
    }

    /// Find keyframe indices.
    pub fn keyframe_indices(&self) -> Vec<u32> {
        self.samples
            .iter()
            .filter(|s| s.is_keyframe)
            .map(|s| s.index)
            .collect()
    }

    /// Find the keyframe at or before the given sample index.
    pub fn find_keyframe_at_or_before(&self, index: u32) -> Option<u32> {
        for i in (0..=index.min(self.sample_count.saturating_sub(1))).rev() {
            if let Some(sample) = self.samples.get(i as usize) {
                if sample.is_keyframe {
                    return Some(i);
                }
            }
        }
        None
    }
}

/// Builder for constructing a sample table from raw atom data.
pub struct SampleTableBuilder {
    // stts: sample duration entries
    stts_entries: Vec<(u32, u32)>, // (count, delta)
    // stss: sync sample numbers (1-based)
    sync_samples: Vec<u32>,
    // stsc: sample-to-chunk entries
    stsc_entries: Vec<(u32, u32, u32)>, // (first_chunk, samples_per_chunk, sample_description_index)
    // stsz: sample sizes (if uniform_size > 0, all samples have that size)
    uniform_size: u32,
    sample_sizes: Vec<u32>,
    // stco/co64: chunk offsets
    chunk_offsets: Vec<u64>,
    // ctts: composition time offsets
    ctts_entries: Vec<(u32, i32)>, // (count, offset)
}

impl SampleTableBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            stts_entries: Vec::new(),
            sync_samples: Vec::new(),
            stsc_entries: Vec::new(),
            uniform_size: 0,
            sample_sizes: Vec::new(),
            chunk_offsets: Vec::new(),
            ctts_entries: Vec::new(),
        }
    }

    /// Set stts (decoding time to sample) entries.
    pub fn set_stts(&mut self, entries: Vec<(u32, u32)>) {
        self.stts_entries = entries;
    }

    /// Set stss (sync sample) entries.
    pub fn set_sync_samples(&mut self, samples: Vec<u32>) {
        self.sync_samples = samples;
    }

    /// Set stsc (sample to chunk) entries.
    pub fn set_stsc(&mut self, entries: Vec<(u32, u32, u32)>) {
        self.stsc_entries = entries;
    }

    /// Set stsz (sample size) data.
    pub fn set_stsz(&mut self, uniform_size: u32, sizes: Vec<u32>) {
        self.uniform_size = uniform_size;
        self.sample_sizes = sizes;
    }

    /// Set chunk offsets (from stco or co64).
    pub fn set_chunk_offsets(&mut self, offsets: Vec<u64>) {
        self.chunk_offsets = offsets;
    }

    /// Set ctts (composition time to sample) entries.
    pub fn set_ctts(&mut self, entries: Vec<(u32, i32)>) {
        self.ctts_entries = entries;
    }

    /// Build the sample table by resolving all sample information.
    pub fn build(self) -> SampleTable {
        let sample_count = if self.uniform_size > 0 {
            self.sample_sizes.len().max(self.total_stts_samples()) as u32
        } else {
            self.sample_sizes.len() as u32
        };

        if sample_count == 0 {
            return SampleTable::default();
        }

        let mut samples = Vec::with_capacity(sample_count as usize);

        // Resolve sample-to-chunk mapping
        let sample_chunks = self.resolve_sample_chunks(sample_count);

        // Resolve sample offsets
        let offsets = self.resolve_offsets(&sample_chunks, sample_count);

        // Resolve sample timestamps
        let (dts_values, _durations) = self.resolve_timestamps(sample_count);

        // Resolve composition time offsets
        let cts_offsets = self.resolve_cts_offsets(sample_count);

        // Create sync sample lookup
        let sync_set: std::collections::HashSet<u32> = self.sync_samples.iter().copied().collect();

        for i in 0..sample_count {
            let size = if self.uniform_size > 0 {
                self.uniform_size
            } else {
                self.sample_sizes.get(i as usize).copied().unwrap_or(0)
            };

            let is_keyframe = if self.sync_samples.is_empty() {
                // No stss means all samples are sync samples
                true
            } else {
                sync_set.contains(&(i + 1)) // stss uses 1-based indexing
            };

            samples.push(SampleEntry {
                index: i,
                offset: offsets.get(i as usize).copied().unwrap_or(0),
                size,
                dts: dts_values.get(i as usize).copied().unwrap_or(0),
                cts_offset: cts_offsets.get(i as usize).copied().unwrap_or(0),
                is_keyframe,
            });
        }

        SampleTable {
            sample_count,
            samples,
        }
    }

    fn total_stts_samples(&self) -> usize {
        self.stts_entries
            .iter()
            .map(|(count, _)| *count as usize)
            .sum()
    }

    fn resolve_sample_chunks(&self, sample_count: u32) -> Vec<u32> {
        if self.stsc_entries.is_empty() {
            return vec![0; sample_count as usize];
        }

        let mut result = Vec::with_capacity(sample_count as usize);
        let mut sample_idx = 0u32;
        let num_chunks = self.chunk_offsets.len() as u32;

        for i in 0..self.stsc_entries.len() {
            let (first_chunk, samples_per_chunk, _) = self.stsc_entries[i];
            let next_first = if i + 1 < self.stsc_entries.len() {
                self.stsc_entries[i + 1].0
            } else {
                num_chunks + 1
            };

            for chunk in first_chunk..next_first {
                if chunk > num_chunks {
                    break;
                }
                for _ in 0..samples_per_chunk {
                    if sample_idx >= sample_count {
                        break;
                    }
                    result.push(chunk - 1); // Convert to 0-based
                    sample_idx += 1;
                }
            }
        }

        // Pad if needed
        while (result.len() as u32) < sample_count {
            result.push(result.last().copied().unwrap_or(0));
        }

        result
    }

    fn resolve_offsets(&self, sample_chunks: &[u32], sample_count: u32) -> Vec<u64> {
        let mut offsets = Vec::with_capacity(sample_count as usize);
        let mut chunk_sample_offset = vec![0u64; self.chunk_offsets.len()];

        for i in 0..sample_count as usize {
            let chunk_idx = sample_chunks.get(i).copied().unwrap_or(0) as usize;
            let chunk_base = self.chunk_offsets.get(chunk_idx).copied().unwrap_or(0);
            let offset = chunk_base + chunk_sample_offset.get(chunk_idx).copied().unwrap_or(0);
            offsets.push(offset);

            // Update offset for next sample in this chunk
            let size = if self.uniform_size > 0 {
                self.uniform_size
            } else {
                self.sample_sizes.get(i).copied().unwrap_or(0)
            };

            if chunk_idx < chunk_sample_offset.len() {
                chunk_sample_offset[chunk_idx] += size as u64;
            }
        }

        offsets
    }

    fn resolve_timestamps(&self, sample_count: u32) -> (Vec<u64>, Vec<u32>) {
        let mut dts_values = Vec::with_capacity(sample_count as usize);
        let mut durations = Vec::with_capacity(sample_count as usize);
        let mut current_dts = 0u64;
        let mut sample_idx = 0u32;

        for (count, delta) in &self.stts_entries {
            for _ in 0..*count {
                if sample_idx >= sample_count {
                    break;
                }
                dts_values.push(current_dts);
                durations.push(*delta);
                current_dts += *delta as u64;
                sample_idx += 1;
            }
        }

        // Pad with last duration if needed
        let last_duration = durations.last().copied().unwrap_or(1);
        while (dts_values.len() as u32) < sample_count {
            dts_values.push(current_dts);
            durations.push(last_duration);
            current_dts += last_duration as u64;
        }

        (dts_values, durations)
    }

    fn resolve_cts_offsets(&self, sample_count: u32) -> Vec<i32> {
        if self.ctts_entries.is_empty() {
            return vec![0; sample_count as usize];
        }

        let mut offsets = Vec::with_capacity(sample_count as usize);
        for (count, offset) in &self.ctts_entries {
            for _ in 0..*count {
                if offsets.len() >= sample_count as usize {
                    break;
                }
                offsets.push(*offset);
            }
        }

        // Pad with zeros if needed
        while (offsets.len() as u32) < sample_count {
            offsets.push(0);
        }

        offsets
    }
}

impl Default for SampleTableBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_entry_pts() {
        let sample = SampleEntry {
            index: 0,
            offset: 100,
            size: 1000,
            dts: 1000,
            cts_offset: 500,
            is_keyframe: true,
        };
        assert_eq!(sample.pts(), 1500);

        let sample_negative = SampleEntry {
            index: 0,
            offset: 100,
            size: 1000,
            dts: 100,
            cts_offset: -200,
            is_keyframe: true,
        };
        assert_eq!(sample_negative.pts(), 0); // Clamped to 0
    }

    #[test]
    fn test_sample_table_builder() {
        let mut builder = SampleTableBuilder::new();

        // 3 samples, each with duration 1000
        builder.set_stts(vec![(3, 1000)]);

        // Sample 1 is a keyframe (1-based)
        builder.set_sync_samples(vec![1]);

        // All samples in chunk 1
        builder.set_stsc(vec![(1, 3, 1)]);

        // Individual sample sizes
        builder.set_stsz(0, vec![100, 200, 150]);

        // One chunk at offset 1000
        builder.set_chunk_offsets(vec![1000]);

        let table = builder.build();

        assert_eq!(table.sample_count, 3);
        assert_eq!(table.samples.len(), 3);

        // Check first sample
        assert_eq!(table.samples[0].index, 0);
        assert_eq!(table.samples[0].offset, 1000);
        assert_eq!(table.samples[0].size, 100);
        assert_eq!(table.samples[0].dts, 0);
        assert!(table.samples[0].is_keyframe);

        // Check second sample
        assert_eq!(table.samples[1].index, 1);
        assert_eq!(table.samples[1].offset, 1100); // 1000 + 100
        assert_eq!(table.samples[1].size, 200);
        assert_eq!(table.samples[1].dts, 1000);
        assert!(!table.samples[1].is_keyframe);

        // Check third sample
        assert_eq!(table.samples[2].offset, 1300); // 1000 + 100 + 200
    }

    #[test]
    fn test_keyframe_search() {
        let mut builder = SampleTableBuilder::new();
        builder.set_stts(vec![(10, 1000)]);
        builder.set_sync_samples(vec![1, 5, 9]); // Keyframes at 0, 4, 8 (0-indexed)
        builder.set_stsc(vec![(1, 10, 1)]);
        builder.set_stsz(100, vec![]); // Uniform size
        builder.set_chunk_offsets(vec![0]);

        let table = builder.build();

        assert_eq!(table.find_keyframe_at_or_before(0), Some(0));
        assert_eq!(table.find_keyframe_at_or_before(3), Some(0));
        assert_eq!(table.find_keyframe_at_or_before(4), Some(4));
        assert_eq!(table.find_keyframe_at_or_before(7), Some(4));
        assert_eq!(table.find_keyframe_at_or_before(8), Some(8));
        assert_eq!(table.find_keyframe_at_or_before(9), Some(8));
    }
}
