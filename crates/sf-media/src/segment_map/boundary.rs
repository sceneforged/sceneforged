//! Pre-computed segment boundaries for HLS serving.
//!
//! The segment map aligns segments to keyframes, targeting a specified duration.
//! This enables zero-copy streaming: the map tells the server exactly which
//! byte ranges to pull from the source file for each HLS segment.

use serde::{Deserialize, Serialize};

/// Information about a single keyframe in the source media.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyframeInfo {
    /// Timestamp of the keyframe in seconds.
    pub timestamp: f64,
    /// Byte offset of the keyframe in the source file.
    pub byte_offset: u64,
}

/// A segment boundary within the media.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentBoundary {
    /// Start time in seconds.
    pub start_time: f64,
    /// End time in seconds.
    pub end_time: f64,
    /// Start byte offset in the source file.
    pub start_byte: u64,
    /// End byte offset in the source file (exclusive).
    pub end_byte: u64,
    /// Whether this segment starts at a keyframe boundary.
    pub is_keyframe_aligned: bool,
}

/// Pre-computed segment map for an entire media file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentMap {
    /// Total duration of the media in seconds.
    pub duration: f64,
    /// Target segment duration in seconds.
    pub target_duration: f64,
    /// Computed segment boundaries.
    pub segments: Vec<SegmentBoundary>,
}

/// Compute a segment map from keyframe positions.
///
/// Segments are aligned to keyframe boundaries, preferring segments close
/// to `target_duration` in length. Each segment starts at a keyframe.
///
/// # Arguments
/// * `keyframes` - Keyframe positions sorted by timestamp.
/// * `total_duration` - Total media duration in seconds.
/// * `target_duration` - Desired segment duration in seconds.
///
/// # Returns
/// A [`SegmentMap`] with segments aligned to the nearest keyframes.
pub fn compute_segment_map(
    keyframes: &[KeyframeInfo],
    total_duration: f64,
    target_duration: f64,
) -> SegmentMap {
    if keyframes.is_empty() || total_duration <= 0.0 || target_duration <= 0.0 {
        return SegmentMap {
            duration: total_duration,
            target_duration,
            segments: vec![],
        };
    }

    let mut segments = Vec::new();
    let mut segment_start_idx = 0usize;

    while segment_start_idx < keyframes.len() {
        let segment_start_time = keyframes[segment_start_idx].timestamp;
        let segment_start_byte = keyframes[segment_start_idx].byte_offset;
        let ideal_end_time = segment_start_time + target_duration;

        // Find the best keyframe to end this segment at.
        // We look for the keyframe whose timestamp is closest to ideal_end_time
        // (but is strictly after segment_start_idx).
        let mut best_end_idx = segment_start_idx + 1;

        if best_end_idx < keyframes.len() {
            let mut best_distance =
                (keyframes[best_end_idx].timestamp - ideal_end_time).abs();

            for i in (segment_start_idx + 2)..keyframes.len() {
                let distance = (keyframes[i].timestamp - ideal_end_time).abs();
                if distance < best_distance {
                    best_distance = distance;
                    best_end_idx = i;
                } else {
                    // Distances are increasing, no point continuing
                    break;
                }
            }
        }

        // Determine the end time and byte offset for this segment
        let (end_time, end_byte) = if best_end_idx < keyframes.len() {
            (
                keyframes[best_end_idx].timestamp,
                keyframes[best_end_idx].byte_offset,
            )
        } else {
            // Last segment: extends to end of file.
            // Use u64::MAX as a sentinel for "end of file" since we don't
            // know the actual file size.
            (total_duration, u64::MAX)
        };

        segments.push(SegmentBoundary {
            start_time: segment_start_time,
            end_time,
            start_byte: segment_start_byte,
            end_byte,
            is_keyframe_aligned: true,
        });

        if best_end_idx >= keyframes.len() {
            break;
        }

        segment_start_idx = best_end_idx;
    }

    SegmentMap {
        duration: total_duration,
        target_duration,
        segments,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_keyframes() {
        let map = compute_segment_map(&[], 10.0, 6.0);
        assert_eq!(map.segments.len(), 0);
        assert_eq!(map.duration, 10.0);
        assert_eq!(map.target_duration, 6.0);
    }

    #[test]
    fn test_single_keyframe() {
        let keyframes = vec![KeyframeInfo {
            timestamp: 0.0,
            byte_offset: 0,
        }];

        let map = compute_segment_map(&keyframes, 5.0, 6.0);
        assert_eq!(map.segments.len(), 1);
        assert_eq!(map.segments[0].start_time, 0.0);
        assert_eq!(map.segments[0].end_time, 5.0);
        assert!(map.segments[0].is_keyframe_aligned);
    }

    #[test]
    fn test_uniform_keyframes_exact_target() {
        // Keyframes exactly at target_duration boundaries
        let keyframes = vec![
            KeyframeInfo { timestamp: 0.0, byte_offset: 0 },
            KeyframeInfo { timestamp: 6.0, byte_offset: 1000 },
            KeyframeInfo { timestamp: 12.0, byte_offset: 2000 },
            KeyframeInfo { timestamp: 18.0, byte_offset: 3000 },
        ];

        let map = compute_segment_map(&keyframes, 24.0, 6.0);

        assert_eq!(map.segments.len(), 4);

        assert_eq!(map.segments[0].start_time, 0.0);
        assert_eq!(map.segments[0].end_time, 6.0);
        assert_eq!(map.segments[0].start_byte, 0);
        assert_eq!(map.segments[0].end_byte, 1000);

        assert_eq!(map.segments[1].start_time, 6.0);
        assert_eq!(map.segments[1].end_time, 12.0);
        assert_eq!(map.segments[1].start_byte, 1000);
        assert_eq!(map.segments[1].end_byte, 2000);

        assert_eq!(map.segments[2].start_time, 12.0);
        assert_eq!(map.segments[2].end_time, 18.0);

        // Last segment extends to end of file
        assert_eq!(map.segments[3].start_time, 18.0);
        assert_eq!(map.segments[3].end_time, 24.0);
    }

    #[test]
    fn test_irregular_keyframes() {
        // Keyframes at irregular intervals, target 6s
        let keyframes = vec![
            KeyframeInfo { timestamp: 0.0, byte_offset: 0 },
            KeyframeInfo { timestamp: 2.0, byte_offset: 500 },
            KeyframeInfo { timestamp: 5.0, byte_offset: 1200 },
            KeyframeInfo { timestamp: 7.0, byte_offset: 1800 },
            KeyframeInfo { timestamp: 10.0, byte_offset: 2500 },
            KeyframeInfo { timestamp: 13.0, byte_offset: 3200 },
        ];

        let map = compute_segment_map(&keyframes, 15.0, 6.0);

        // All segments should start at keyframes
        for seg in &map.segments {
            assert!(seg.is_keyframe_aligned);
        }

        // First segment: 0.0 -> closest to 6.0 among [2.0, 5.0, 7.0, ...]
        // 5.0 is 1.0 away, 7.0 is 1.0 away. 5.0 comes first in scan but 7.0 has same distance.
        // Actually 5.0 has distance 1.0, 7.0 has distance 1.0. We pick the first minimum found.
        // Since we break when distance increases, and |5-6|=1, |7-6|=1, |10-6|=4 (increases),
        // best_end_idx will be at timestamp 5.0.
        assert_eq!(map.segments[0].start_time, 0.0);
        assert_eq!(map.segments[0].end_time, 5.0);

        // Segments should cover all content
        assert_eq!(map.segments.last().unwrap().end_time, 15.0);
    }

    #[test]
    fn test_short_total_duration() {
        let keyframes = vec![
            KeyframeInfo { timestamp: 0.0, byte_offset: 0 },
            KeyframeInfo { timestamp: 2.0, byte_offset: 500 },
        ];

        let map = compute_segment_map(&keyframes, 3.0, 6.0);

        // With target 6s and only 3s of content, should be 1-2 segments
        assert!(!map.segments.is_empty());
        assert_eq!(map.segments.last().unwrap().end_time, 3.0);
    }

    #[test]
    fn test_many_keyframes_close_together() {
        // 1 keyframe per second, target 6s
        let keyframes: Vec<KeyframeInfo> = (0..30)
            .map(|i| KeyframeInfo {
                timestamp: i as f64,
                byte_offset: i as u64 * 1000,
            })
            .collect();

        let map = compute_segment_map(&keyframes, 30.0, 6.0);

        // Interior segments should be approximately 6 seconds.
        // The last segment may be shorter since it extends to end of file.
        for (i, seg) in map.segments.iter().enumerate() {
            let dur = seg.end_time - seg.start_time;
            if i < map.segments.len() - 1 {
                assert!(
                    dur >= 5.0 && dur <= 7.0,
                    "Interior segment {} duration {} out of expected range",
                    i, dur
                );
            } else {
                // Last segment can be shorter
                assert!(
                    dur > 0.0 && dur <= 7.0,
                    "Final segment duration {} out of expected range",
                    dur
                );
            }
        }
    }

    #[test]
    fn test_zero_duration() {
        let keyframes = vec![KeyframeInfo {
            timestamp: 0.0,
            byte_offset: 0,
        }];

        let map = compute_segment_map(&keyframes, 0.0, 6.0);
        assert_eq!(map.segments.len(), 0);
    }

    #[test]
    fn test_zero_target_duration() {
        let keyframes = vec![KeyframeInfo {
            timestamp: 0.0,
            byte_offset: 0,
        }];

        let map = compute_segment_map(&keyframes, 10.0, 0.0);
        assert_eq!(map.segments.len(), 0);
    }

    #[test]
    fn test_segment_boundaries_are_contiguous() {
        let keyframes: Vec<KeyframeInfo> = (0..10)
            .map(|i| KeyframeInfo {
                timestamp: i as f64 * 3.0,
                byte_offset: i as u64 * 5000,
            })
            .collect();

        let map = compute_segment_map(&keyframes, 30.0, 6.0);

        // Verify segments are contiguous in time
        for i in 1..map.segments.len() {
            assert_eq!(
                map.segments[i].start_time,
                map.segments[i - 1].end_time,
                "Gap between segment {} and {}",
                i - 1,
                i
            );
        }

        // First segment starts at 0
        assert_eq!(map.segments[0].start_time, 0.0);
        // Last segment ends at total duration
        assert_eq!(map.segments.last().unwrap().end_time, 30.0);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let map = compute_segment_map(
            &[
                KeyframeInfo { timestamp: 0.0, byte_offset: 0 },
                KeyframeInfo { timestamp: 6.0, byte_offset: 1000 },
            ],
            12.0,
            6.0,
        );

        let json = serde_json::to_string(&map).unwrap();
        let deserialized: SegmentMap = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.duration, map.duration);
        assert_eq!(deserialized.target_duration, map.target_duration);
        assert_eq!(deserialized.segments.len(), map.segments.len());
    }
}
