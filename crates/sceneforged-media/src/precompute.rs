//! HLS precomputation.
//!
//! Precomputes all data needed for HLS fMP4 serving at scan/conversion time,
//! so that runtime serving is just database lookups and file byte-range reads.

use crate::fmp4::{build_multi_track_moof, InitSegmentBuilder, MoofBuilder, TrackFragment};
use crate::mp4::{Mp4File, SampleTable, TrackInfo};
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
/// 2. Builds the SegmentMap with keyframe-aligned boundaries
/// 3. Pre-builds moof+mdat headers for each segment (video + audio)
/// 4. Builds the fMP4 init segment with real codec config
///
/// Returns an error if the file can't be parsed or has no video track.
pub fn precompute_hls(path: &Path) -> Result<HlsPrecomputed> {
    let mp4 = Mp4File::open(path)?;

    let video_track = mp4
        .video_track
        .as_ref()
        .ok_or_else(|| crate::Error::InvalidMp4("No video track found".into()))?;

    // Build segment map from video track
    let mut segment_map = SegmentMapBuilder::new()
        .timescale(video_track.timescale)
        .target_duration(6.0)
        .build(&video_track.sample_table);

    // Compute audio sample ranges per segment if audio track exists
    if let Some(ref audio_track) = mp4.audio_track {
        compute_audio_ranges(&mut segment_map, video_track, audio_track);
    }

    // Pre-build moof+mdat headers for each segment
    for segment in &mut segment_map.segments {
        let video_samples: Vec<_> = (segment.start_sample..segment.end_sample)
            .filter_map(|i| video_track.sample_table.get(i))
            .cloned()
            .collect();

        let video_base_time = video_samples.first().map(|s| s.dts).unwrap_or(0);

        // Check if we have audio for this segment
        let has_audio = segment.audio_start_sample.is_some()
            && mp4.audio_track.is_some()
            && segment.audio_start_sample != segment.audio_end_sample;

        if has_audio {
            let audio_track = mp4.audio_track.as_ref().unwrap();
            let audio_start = segment.audio_start_sample.unwrap();
            let audio_end = segment.audio_end_sample.unwrap();

            let audio_samples: Vec<_> = (audio_start..audio_end)
                .filter_map(|i| audio_track.sample_table.get(i))
                .cloned()
                .collect();

            let audio_base_time = audio_samples.first().map(|s| s.dts).unwrap_or(0);

            let tracks = [
                TrackFragment {
                    track_id: 1,
                    base_media_decode_time: video_base_time,
                    samples: &video_samples,
                },
                TrackFragment {
                    track_id: 2,
                    base_media_decode_time: audio_base_time,
                    samples: &audio_samples,
                },
            ];

            segment.moof_data = Some(build_multi_track_moof(segment.index + 1, &tracks));
        } else {
            // Video-only segment
            let moof_data = MoofBuilder::new(segment.index + 1, 1)
                .base_media_decode_time(video_base_time)
                .build(&video_samples);
            segment.moof_data = Some(moof_data);
        }
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
        init_builder = init_builder.with_audio(
            audio_track.timescale,
            channels,
            sample_rate,
            audio_track.duration,
        );
        if let Some(ref codec_data) = audio_track.codec_data {
            init_builder = init_builder.audio_codec(codec_data.clone());
        }
    }

    let init_segment = init_builder.build();

    // Store init segment in the segment map too
    segment_map.init_segment = Some(init_segment.data.clone());

    Ok(HlsPrecomputed {
        init_segment: init_segment.data,
        segment_map,
    })
}

/// Compute audio sample ranges and byte ranges for each video segment.
///
/// For each video segment, finds the audio samples that fall within the same
/// time range by converting video segment boundaries to audio timescale.
fn compute_audio_ranges(
    segment_map: &mut crate::segment_map::SegmentMap,
    _video_track: &TrackInfo,
    audio_track: &TrackInfo,
) {
    let audio_samples = &audio_track.sample_table;
    if audio_samples.samples.is_empty() {
        return;
    }

    let audio_ts = audio_track.timescale as f64;

    for segment in &mut segment_map.segments {
        // Convert video segment time boundaries to seconds, then to audio timescale
        let start_secs = segment.start_time_secs;
        let end_secs = segment.start_time_secs + segment.duration_secs;

        let audio_start_dts = (start_secs * audio_ts) as u64;
        let audio_end_dts = (end_secs * audio_ts) as u64;

        // Find audio sample range by DTS
        let audio_start_idx = find_audio_sample_at_or_after(audio_samples, audio_start_dts);
        let audio_end_idx = find_audio_sample_at_or_after(audio_samples, audio_end_dts);

        if audio_start_idx < audio_end_idx {
            segment.audio_start_sample = Some(audio_start_idx);
            segment.audio_end_sample = Some(audio_end_idx);

            // Coalesce audio byte ranges
            let mut byte_ranges: Vec<(u64, u32)> = Vec::new();
            for i in audio_start_idx..audio_end_idx {
                if let Some(sample) = audio_samples.get(i) {
                    if let Some(last) = byte_ranges.last_mut() {
                        let run_end = last.0 + last.1 as u64;
                        if sample.offset == run_end {
                            last.1 += sample.size;
                            continue;
                        }
                    }
                    byte_ranges.push((sample.offset, sample.size));
                }
            }
            segment.audio_byte_ranges = byte_ranges;
        }
    }

    // Ensure the last segment includes any remaining audio samples
    if let Some(last_segment) = segment_map.segments.last_mut() {
        let current_end = last_segment.audio_end_sample.unwrap_or(0);
        if current_end < audio_samples.sample_count {
            let start = last_segment.audio_start_sample.unwrap_or(current_end);
            last_segment.audio_start_sample = Some(start);
            last_segment.audio_end_sample = Some(audio_samples.sample_count);

            // Recompute byte ranges for the extended range
            let mut byte_ranges: Vec<(u64, u32)> = Vec::new();
            for i in start..audio_samples.sample_count {
                if let Some(sample) = audio_samples.get(i) {
                    if let Some(last) = byte_ranges.last_mut() {
                        let run_end = last.0 + last.1 as u64;
                        if sample.offset == run_end {
                            last.1 += sample.size;
                            continue;
                        }
                    }
                    byte_ranges.push((sample.offset, sample.size));
                }
            }
            last_segment.audio_byte_ranges = byte_ranges;
        }
    }
}

/// Find the index of the first audio sample with DTS >= target_dts.
fn find_audio_sample_at_or_after(sample_table: &SampleTable, target_dts: u64) -> u32 {
    let samples = &sample_table.samples;
    match samples.binary_search_by_key(&target_dts, |s| s.dts) {
        Ok(idx) => idx as u32,
        Err(idx) => idx as u32,
    }
}
