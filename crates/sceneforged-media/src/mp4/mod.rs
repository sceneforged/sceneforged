//! MP4 container parsing.
//!
//! This module provides structures for parsing MP4 files to extract
//! the information needed for HLS segment generation.

mod atoms;
mod reader;
mod sample_table;

pub use atoms::{Atom, AtomType, HandlerType, TrackInfo};
pub use reader::Mp4Reader;
pub use sample_table::{SampleEntry, SampleTable, SampleTableBuilder};

use crate::Result;
use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::Path;

/// Parsed MP4 file with extracted sample tables.
#[derive(Debug)]
pub struct Mp4File {
    /// Duration in timescale units.
    pub duration: u64,
    /// Movie timescale (time units per second).
    pub timescale: u32,
    /// Video track information.
    pub video_track: Option<TrackInfo>,
    /// Audio track information.
    pub audio_track: Option<TrackInfo>,
    /// Whether the file has faststart (moov before mdat).
    pub has_faststart: bool,
}

impl Mp4File {
    /// Parse an MP4 file from the given path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        Self::parse(&mut reader)
    }

    /// Parse an MP4 file from a reader.
    pub fn parse<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        let mut mp4_reader = Mp4Reader::new(reader);
        mp4_reader.parse()
    }

    /// Get the duration in seconds.
    pub fn duration_secs(&self) -> f64 {
        if self.timescale == 0 {
            0.0
        } else {
            self.duration as f64 / self.timescale as f64
        }
    }

    /// Get the video sample table, if available.
    pub fn video_samples(&self) -> Option<&SampleTable> {
        self.video_track.as_ref().map(|t| &t.sample_table)
    }

    /// Get the audio sample table, if available.
    pub fn audio_samples(&self) -> Option<&SampleTable> {
        self.audio_track.as_ref().map(|t| &t.sample_table)
    }

    /// Calculate the maximum keyframe interval in seconds.
    ///
    /// Returns the largest gap between consecutive keyframes in the video track.
    /// This is useful for validating HLS compatibility (typically ≤2s required).
    ///
    /// Returns `None` if there is no video track or fewer than 2 keyframes.
    pub fn max_keyframe_interval_secs(&self) -> Option<f64> {
        let video = self.video_track.as_ref()?;
        let timescale = video.timescale;
        if timescale == 0 {
            return None;
        }

        let samples = &video.sample_table.samples;
        if samples.is_empty() {
            return None;
        }

        // Find all keyframe DTS values
        let keyframe_dts: Vec<u64> = samples
            .iter()
            .filter(|s| s.is_keyframe)
            .map(|s| s.dts)
            .collect();

        if keyframe_dts.len() < 2 {
            // With 0 or 1 keyframe, we can't determine an interval
            // Return None to indicate unknown
            return None;
        }

        // Find the maximum interval between consecutive keyframes
        let max_interval = keyframe_dts
            .windows(2)
            .map(|w| w[1].saturating_sub(w[0]))
            .max()?;

        Some(max_interval as f64 / timescale as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mp4_file_duration() {
        let mp4 = Mp4File {
            duration: 120000,
            timescale: 1000,
            video_track: None,
            audio_track: None,
            has_faststart: true,
        };
        assert!((mp4.duration_secs() - 120.0).abs() < 0.001);
    }

    #[test]
    fn test_mp4_file_zero_timescale() {
        let mp4 = Mp4File {
            duration: 120000,
            timescale: 0,
            video_track: None,
            audio_track: None,
            has_faststart: true,
        };
        assert_eq!(mp4.duration_secs(), 0.0);
    }

    #[test]
    fn test_max_keyframe_interval_no_video() {
        let mp4 = Mp4File {
            duration: 120000,
            timescale: 1000,
            video_track: None,
            audio_track: None,
            has_faststart: true,
        };
        assert!(mp4.max_keyframe_interval_secs().is_none());
    }

    #[test]
    fn test_max_keyframe_interval_calculation() {
        use super::sample_table::{SampleEntry, SampleTable};
        use super::atoms::{HandlerType, TrackInfo};

        // Create samples with keyframes at 0s, 2s, 5s (max interval = 3s)
        let samples = vec![
            SampleEntry { index: 0, offset: 0, size: 100, dts: 0, cts_offset: 0, is_keyframe: true },
            SampleEntry { index: 1, offset: 100, size: 100, dts: 1000, cts_offset: 0, is_keyframe: false },
            SampleEntry { index: 2, offset: 200, size: 100, dts: 2000, cts_offset: 0, is_keyframe: true },
            SampleEntry { index: 3, offset: 300, size: 100, dts: 3000, cts_offset: 0, is_keyframe: false },
            SampleEntry { index: 4, offset: 400, size: 100, dts: 4000, cts_offset: 0, is_keyframe: false },
            SampleEntry { index: 5, offset: 500, size: 100, dts: 5000, cts_offset: 0, is_keyframe: true },
        ];

        let sample_table = SampleTable {
            sample_count: 6,
            samples,
        };

        let track = TrackInfo {
            track_id: 1,
            handler_type: HandlerType::Video,
            duration: 5000,
            timescale: 1000, // 1000 units per second
            sample_table,
            codec_data: None,
            width: Some(1920),
            height: Some(1080),
            sample_rate: None,
            channels: None,
        };

        let mp4 = Mp4File {
            duration: 5000,
            timescale: 1000,
            video_track: Some(track),
            audio_track: None,
            has_faststart: true,
        };

        let interval = mp4.max_keyframe_interval_secs();
        assert!(interval.is_some());
        // Max interval is between 2s and 5s = 3 seconds
        assert!((interval.unwrap() - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_max_keyframe_interval_single_keyframe() {
        use super::sample_table::{SampleEntry, SampleTable};
        use super::atoms::{HandlerType, TrackInfo};

        let samples = vec![
            SampleEntry { index: 0, offset: 0, size: 100, dts: 0, cts_offset: 0, is_keyframe: true },
            SampleEntry { index: 1, offset: 100, size: 100, dts: 1000, cts_offset: 0, is_keyframe: false },
        ];

        let sample_table = SampleTable {
            sample_count: 2,
            samples,
        };

        let track = TrackInfo {
            track_id: 1,
            handler_type: HandlerType::Video,
            duration: 1000,
            timescale: 1000,
            sample_table,
            codec_data: None,
            width: Some(1920),
            height: Some(1080),
            sample_rate: None,
            channels: None,
        };

        let mp4 = Mp4File {
            duration: 1000,
            timescale: 1000,
            video_track: Some(track),
            audio_track: None,
            has_faststart: true,
        };

        // Only one keyframe, can't determine interval
        assert!(mp4.max_keyframe_interval_secs().is_none());
    }
}
