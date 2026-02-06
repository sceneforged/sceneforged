//! Build a `PreparedMedia` from parsed MP4 metadata.
//!
//! This is the core of zero-copy HLS serving: it precomputes moof boxes and
//! data ranges so that at serve time, the server only needs to concatenate
//! pre-built bytes from RAM with sample data read from the source file.

use std::path::Path;

use crate::fmp4::boxes::{
    self, TrunSampleFull, TrunSampleSimple,
};
use crate::fmp4::{Codec, TrackConfig};
use crate::hls::{generate_media_playlist, MediaPlaylist, Segment};
use crate::mp4::Mp4Metadata;

use super::types::{DataRange, PrecomputedSegment, PreparedMedia};

/// Target HLS segment duration in seconds.
const TARGET_SEGMENT_SECS: f64 = 6.0;

/// Build a fully prepared media file for zero-copy HLS serving.
pub fn build_prepared_media(
    metadata: &Mp4Metadata,
    file_path: &Path,
) -> Result<PreparedMedia, String> {
    let video = metadata
        .video_track
        .as_ref()
        .ok_or("No video track found")?;
    let audio = metadata.audio_track.as_ref();

    // Build init segment.
    let video_config = TrackConfig {
        track_id: 1,
        timescale: video.timescale,
        codec: Codec::Avc,
        width: video.width,
        height: video.height,
        sample_rate: 0,
        channels: 0,
        codec_private: video.codec_private.clone(),
    };

    let init_segment = if let Some(audio_track) = audio {
        let audio_config = TrackConfig {
            track_id: 2,
            timescale: audio_track.timescale,
            codec: Codec::Aac,
            width: 0,
            height: 0,
            sample_rate: audio_track.sample_rate,
            channels: audio_track.channels,
            codec_private: audio_track.codec_private.clone(),
        };
        crate::fmp4::write_init_segment_multi(&video_config, &audio_config)
    } else {
        crate::fmp4::write_init_segment(&video_config)
    };

    // Compute segment boundaries from video keyframes.
    let video_samples = &video.sample_table.samples;
    if video_samples.is_empty() {
        return Err("Video track has no samples".into());
    }

    let video_ts = video.timescale as f64;

    // Find keyframe indices.
    let keyframe_indices: Vec<usize> = video_samples
        .iter()
        .enumerate()
        .filter(|(_, s)| s.is_sync)
        .map(|(i, _)| i)
        .collect();

    if keyframe_indices.is_empty() {
        return Err("No keyframes found in video track".into());
    }

    // Build segment boundaries: each segment starts at a keyframe.
    let target_ticks = (TARGET_SEGMENT_SECS * video_ts) as u64;
    let mut segment_ranges: Vec<(usize, usize)> = Vec::new(); // (start_sample, end_sample) exclusive
    let mut seg_start_kf = 0usize; // index into keyframe_indices

    while seg_start_kf < keyframe_indices.len() {
        let start_sample = keyframe_indices[seg_start_kf];
        let start_dts = video_samples[start_sample].decode_timestamp;
        let target_end_dts = start_dts + target_ticks;

        // Find the next keyframe at or after target.
        let mut best_kf = seg_start_kf + 1;
        if best_kf < keyframe_indices.len() {
            let mut best_dist = (video_samples[keyframe_indices[best_kf]].decode_timestamp as i64
                - target_end_dts as i64)
                .unsigned_abs();

            for kf_idx in (seg_start_kf + 2)..keyframe_indices.len() {
                let dist = (video_samples[keyframe_indices[kf_idx]].decode_timestamp as i64
                    - target_end_dts as i64)
                    .unsigned_abs();
                if dist < best_dist {
                    best_dist = dist;
                    best_kf = kf_idx;
                } else {
                    break;
                }
            }
        }

        let end_sample = if best_kf < keyframe_indices.len() {
            keyframe_indices[best_kf]
        } else {
            video_samples.len()
        };

        segment_ranges.push((start_sample, end_sample));

        if best_kf >= keyframe_indices.len() {
            break;
        }
        seg_start_kf = best_kf;
    }

    // Build precomputed segments.
    let mut segments = Vec::with_capacity(segment_ranges.len());

    for (seg_idx, &(vs_start, vs_end)) in segment_ranges.iter().enumerate() {
        let seg_video_samples = &video_samples[vs_start..vs_end];
        if seg_video_samples.is_empty() {
            continue;
        }

        let video_base_dts = seg_video_samples[0].decode_timestamp;
        let video_end_dts = if vs_end < video_samples.len() {
            video_samples[vs_end].decode_timestamp
        } else {
            // Last sample: end = last DTS + last duration.
            let last = seg_video_samples.last().unwrap();
            last.decode_timestamp + last.duration as u64
        };

        let start_time_secs = video_base_dts as f64 / video_ts;
        let end_time_secs = video_end_dts as f64 / video_ts;
        let duration_secs = end_time_secs - start_time_secs;

        // Find corresponding audio samples by time range.
        let audio_data = audio.map(|audio_track| {
            let audio_ts = audio_track.timescale as f64;
            let audio_start_time = video_base_dts as f64 / video_ts;
            let audio_end_time = video_end_dts as f64 / video_ts;
            let audio_start_tick = (audio_start_time * audio_ts) as u64;
            let audio_end_tick = (audio_end_time * audio_ts) as u64;

            let audio_samples = &audio_track.sample_table.samples;
            let as_start = audio_samples
                .iter()
                .position(|s| s.decode_timestamp >= audio_start_tick)
                .unwrap_or(audio_samples.len());
            let as_end = audio_samples
                .iter()
                .position(|s| s.decode_timestamp >= audio_end_tick)
                .unwrap_or(audio_samples.len());

            (audio_track, &audio_samples[as_start..as_end], as_start, audio_start_tick)
        });

        // Collect all sample data ranges, sorted by file offset.
        let mut all_ranges: Vec<DataRange> = Vec::new();
        let mut video_data_size: u64 = 0;
        for s in seg_video_samples {
            all_ranges.push(DataRange {
                file_offset: s.file_offset,
                length: s.size as u64,
            });
            video_data_size += s.size as u64;
        }

        let mut audio_data_size: u64 = 0;
        if let Some((_, audio_seg_samples, _, _)) = &audio_data {
            for s in *audio_seg_samples {
                all_ranges.push(DataRange {
                    file_offset: s.file_offset,
                    length: s.size as u64,
                });
                audio_data_size += s.size as u64;
            }
        }

        // Sort by offset and merge adjacent/overlapping ranges.
        all_ranges.sort_by_key(|r| r.file_offset);
        let merged_ranges = merge_data_ranges(&all_ranges);

        let total_data_size = video_data_size + audio_data_size;

        // Build moof box.
        let mfhd = boxes::write_mfhd((seg_idx + 1) as u32);

        // Video traf.
        let video_tfhd = boxes::write_tfhd(1); // track_id=1
        let video_tfdt = boxes::write_tfdt(video_base_dts);
        let video_trun_samples: Vec<TrunSampleFull> = seg_video_samples
            .iter()
            .map(|s| TrunSampleFull {
                duration: s.duration,
                size: s.size,
                flags: if s.is_sync {
                    0x02000000
                } else {
                    0x01010000
                },
                composition_time_offset: s.composition_offset,
            })
            .collect();

        // We'll compute data_offset after we know the moof size.
        // For now, use a placeholder and then fix up.
        // trun_full content: fullbox(4) + count(4) + offset(4) + samples*16
        let video_trun_content = 4 + 4 + 4 + video_trun_samples.len() * 16;
        let video_trun_box_size = 8 + video_trun_content;
        let video_traf_size =
            8 + video_tfhd.len() + video_tfdt.len() + video_trun_box_size;

        // Audio traf (optional).
        let audio_traf_components = audio_data.as_ref().map(|(audio_track, audio_seg_samples, _, _)| {
            let audio_tfhd = boxes::write_tfhd(2); // track_id=2
            let audio_base_dts = if !audio_seg_samples.is_empty() {
                audio_seg_samples[0].decode_timestamp
            } else {
                let audio_ts = audio_track.timescale as f64;
                (start_time_secs * audio_ts) as u64
            };
            let audio_tfdt = boxes::write_tfdt(audio_base_dts);
            let audio_trun_samples: Vec<TrunSampleSimple> = audio_seg_samples
                .iter()
                .map(|s| TrunSampleSimple {
                    duration: s.duration,
                    size: s.size,
                })
                .collect();
            // trun_simple content: fullbox(4) + count(4) + offset(4) + samples*8
            let audio_trun_content = 4 + 4 + 4 + audio_trun_samples.len() * 8;
            let audio_trun_box_size = 8 + audio_trun_content;
            let audio_traf_size =
                8 + audio_tfhd.len() + audio_tfdt.len() + audio_trun_box_size;
            (audio_tfhd, audio_tfdt, audio_trun_samples, audio_traf_size)
        });

        let audio_traf_total_size = audio_traf_components
            .as_ref()
            .map(|(_, _, _, size)| *size)
            .unwrap_or(0);

        // moof size = 8 + mfhd + video_traf + audio_traf
        let moof_size = 8 + mfhd.len() + video_traf_size + audio_traf_total_size;

        // mdat header size.
        let mdat_hdr = boxes::write_mdat_header(total_data_size);
        let mdat_hdr_size = mdat_hdr.len();

        // Video data offset: from start of moof to first video sample data.
        // = moof_size + mdat_hdr_size
        let video_data_offset = (moof_size + mdat_hdr_size) as i32;

        // Audio data offset: video_data_offset + video_data_size
        let audio_data_offset = video_data_offset + video_data_size as i32;

        // Build the actual trun boxes with correct offsets.
        let video_trun = boxes::write_trun_full(&video_trun_samples, video_data_offset);
        let video_traf =
            boxes::write_container_box(b"traf", &[&video_tfhd, &video_tfdt, &video_trun]);

        let moof = if let Some((audio_tfhd, audio_tfdt, audio_trun_samples, _)) =
            &audio_traf_components
        {
            let audio_trun = boxes::write_trun_simple(audio_trun_samples, audio_data_offset);
            let audio_traf =
                boxes::write_container_box(b"traf", &[audio_tfhd, audio_tfdt, &audio_trun]);
            boxes::write_container_box(b"moof", &[&mfhd, &video_traf, &audio_traf])
        } else {
            boxes::write_container_box(b"moof", &[&mfhd, &video_traf])
        };

        segments.push(PrecomputedSegment {
            index: seg_idx as u32,
            start_time_secs,
            duration_secs,
            moof_bytes: moof,
            mdat_header: mdat_hdr,
            data_ranges: merged_ranges,
            data_length: total_data_size,
        });
    }

    // Build variant playlist.
    let max_duration = segments
        .iter()
        .map(|s| s.duration_secs)
        .fold(0.0f64, f64::max);
    let target_duration = max_duration.ceil() as u32;

    let hls_segments: Vec<Segment> = segments
        .iter()
        .map(|s| Segment {
            duration: s.duration_secs,
            uri: format!("segment_{}.m4s", s.index),
            title: None,
        })
        .collect();

    let playlist = MediaPlaylist {
        target_duration,
        media_sequence: 0,
        segments: hls_segments,
        ended: true,
        init_segment_uri: Some("init.mp4".to_string()),
    };
    let variant_playlist = generate_media_playlist(&playlist);

    Ok(PreparedMedia {
        file_path: file_path.to_path_buf(),
        width: video.width,
        height: video.height,
        duration_secs: metadata.duration_secs,
        init_segment,
        variant_playlist,
        segments,
        target_duration,
    })
}

/// Merge adjacent or overlapping data ranges.
fn merge_data_ranges(ranges: &[DataRange]) -> Vec<DataRange> {
    if ranges.is_empty() {
        return Vec::new();
    }
    let mut merged = Vec::with_capacity(ranges.len());
    let mut current = DataRange {
        file_offset: ranges[0].file_offset,
        length: ranges[0].length,
    };

    for r in &ranges[1..] {
        let current_end = current.file_offset + current.length;
        if r.file_offset <= current_end {
            // Overlapping or adjacent: extend.
            let new_end = (r.file_offset + r.length).max(current_end);
            current.length = new_end - current.file_offset;
        } else {
            merged.push(current);
            current = DataRange {
                file_offset: r.file_offset,
                length: r.length,
            };
        }
    }
    merged.push(current);
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_data_ranges_adjacent() {
        let ranges = vec![
            DataRange { file_offset: 100, length: 50 },
            DataRange { file_offset: 150, length: 30 },
        ];
        let merged = merge_data_ranges(&ranges);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].file_offset, 100);
        assert_eq!(merged[0].length, 80);
    }

    #[test]
    fn test_merge_data_ranges_gap() {
        let ranges = vec![
            DataRange { file_offset: 100, length: 50 },
            DataRange { file_offset: 200, length: 30 },
        ];
        let merged = merge_data_ranges(&ranges);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn test_merge_data_ranges_overlapping() {
        let ranges = vec![
            DataRange { file_offset: 100, length: 50 },
            DataRange { file_offset: 120, length: 50 },
        ];
        let merged = merge_data_ranges(&ranges);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].file_offset, 100);
        assert_eq!(merged[0].length, 70);
    }
}
