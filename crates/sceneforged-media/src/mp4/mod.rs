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
}
