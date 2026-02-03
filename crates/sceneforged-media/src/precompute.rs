//! HLS precomputation.
//!
//! Precomputes all data needed for HLS fMP4 serving at scan/conversion time,
//! so that runtime serving is just database lookups and file byte-range reads.

use crate::fmp4::{InitSegmentBuilder, MoofBuilder};
use crate::mp4::Mp4File;
use crate::segment_map::SegmentMapBuilder;
use crate::Result;
use std::path::Path;

/// Precomputed HLS data ready for database storage.
pub struct HlsPrecomputed {
    /// fMP4 init segment (ftyp + moov).
    pub init_segment: Vec<u8>,
    /// Serialized SegmentMap with pre-built moof_data per segment.
    pub segment_map: crate::segment_map::SegmentMap,
}

/// Precompute all HLS serving data for a Profile B MP4 file.
///
/// This function:
/// 1. Parses the MP4 to extract sample tables
/// 2. Validates video sample contiguity (required for zero-copy serving)
/// 3. Builds the SegmentMap with keyframe-aligned boundaries
/// 4. Pre-builds moof+mdat headers for each segment
/// 5. Builds the fMP4 init segment with real codec config
///
/// Returns an error if the file can't be parsed, has no video track,
/// or has non-contiguous video samples.
pub fn precompute_hls(path: &Path) -> Result<HlsPrecomputed> {
    let mp4 = Mp4File::open(path)?;

    let video_track = mp4
        .video_track
        .as_ref()
        .ok_or_else(|| crate::Error::InvalidMp4("No video track found".into()))?;

    // Validate sample contiguity
    validate_contiguity(&video_track.sample_table)?;

    // Build segment map
    let mut segment_map = SegmentMapBuilder::new()
        .timescale(video_track.timescale)
        .target_duration(6.0)
        .build(&video_track.sample_table);

    // Pre-build moof+mdat headers for each segment
    for segment in &mut segment_map.segments {
        let samples: Vec<_> = (segment.start_sample..segment.end_sample)
            .filter_map(|i| video_track.sample_table.get(i))
            .cloned()
            .collect();

        let base_decode_time = samples.first().map(|s| s.dts).unwrap_or(0);

        let moof_data = MoofBuilder::new(segment.index + 1, 1)
            .base_media_decode_time(base_decode_time)
            .build(&samples);

        segment.moof_data = Some(moof_data);
    }

    // Build init segment
    let mut init_builder = InitSegmentBuilder::new()
        .timescale(video_track.timescale)
        .duration(video_track.duration);

    if let (Some(width), Some(height)) = (video_track.width, video_track.height) {
        init_builder = init_builder.dimensions(width, height);
    }

    if let Some(ref codec_data) = video_track.codec_data {
        init_builder = init_builder.video_codec(codec_data.clone());
    }

    // Add audio track info if present
    if let Some(ref audio_track) = mp4.audio_track {
        let channels = audio_track.channels.unwrap_or(2);
        let sample_rate = audio_track.sample_rate.unwrap_or(48000);
        init_builder = init_builder.with_audio(audio_track.timescale, channels, sample_rate);
    }

    let init_segment = init_builder.build();

    // Store init segment in the segment map too
    segment_map.init_segment = Some(init_segment.data.clone());

    Ok(HlsPrecomputed {
        init_segment: init_segment.data,
        segment_map,
    })
}

/// Validate that video samples are contiguous in the file.
///
/// For zero-copy HLS serving, we seek to the first sample's offset and read
/// `data_size` bytes. If samples aren't contiguous, we'd serve wrong data.
fn validate_contiguity(sample_table: &crate::mp4::SampleTable) -> Result<()> {
    if sample_table.samples.len() < 2 {
        return Ok(());
    }

    for window in sample_table.samples.windows(2) {
        let current = &window[0];
        let next = &window[1];
        let expected_next_offset = current.offset + current.size as u64;

        if next.offset != expected_next_offset {
            return Err(crate::Error::InvalidMp4(format!(
                "Non-contiguous video samples at index {}: expected offset {}, got {}",
                next.index, expected_next_offset, next.offset
            )));
        }
    }

    Ok(())
}
