//! High-level fMP4 writer functions.
//!
//! Provides `write_init_segment` and `write_media_segment` for generating
//! fragmented MP4 data suitable for HLS streaming.

use super::boxes::{self, Codec, TrunSample};
use serde::{Deserialize, Serialize};

/// Configuration for a track in an fMP4 init segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackConfig {
    /// Track ID (1-based).
    pub track_id: u32,
    /// Media timescale (ticks per second).
    pub timescale: u32,
    /// Codec used by this track.
    pub codec: Codec,
    /// Video width in pixels (0 for audio tracks).
    pub width: u32,
    /// Video height in pixels (0 for audio tracks).
    pub height: u32,
    /// Audio sample rate in Hz (0 for video tracks).
    pub sample_rate: u32,
    /// Audio channel count (0 for video tracks).
    pub channels: u16,
    /// Codec-specific configuration data (e.g. avcC, hvcC, or esds bytes).
    pub codec_private: Vec<u8>,
}

/// Information about a single sample (frame/packet) in a media segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleInfo {
    /// Raw sample data.
    pub data: Vec<u8>,
    /// Sample duration in timescale units.
    pub duration: u32,
    /// Whether this sample is a keyframe (sync sample).
    pub is_keyframe: bool,
    /// Composition time offset (signed, for B-frames).
    pub composition_offset: i32,
}

/// Generate an fMP4 initialization segment (ftyp + moov).
///
/// The init segment describes the track structure and codec configuration.
/// It must be sent before any media segments.
pub fn write_init_segment(config: &TrackConfig) -> Vec<u8> {
    let ftyp = boxes::write_ftyp();

    let is_video = matches!(config.codec, Codec::Avc | Codec::Hevc);

    let trak = if is_video {
        boxes::write_video_trak(
            config.track_id,
            config.timescale,
            0, // duration unknown for fragmented
            &config.codec,
            config.width,
            config.height,
            &config.codec_private,
        )
    } else {
        boxes::write_audio_trak(
            config.track_id,
            config.timescale,
            0,
            config.sample_rate,
            config.channels,
            &config.codec_private,
        )
    };

    let mvex = boxes::write_mvex(config.track_id);
    let moov = boxes::write_moov(config.timescale, 0, &trak, &mvex);

    let mut result = Vec::with_capacity(ftyp.len() + moov.len());
    result.extend_from_slice(&ftyp);
    result.extend_from_slice(&moov);
    result
}

/// Generate an fMP4 media segment (moof + mdat).
///
/// Each media segment contains a movie fragment header describing the samples,
/// followed by the concatenated sample data.
///
/// # Arguments
/// * `seq` - Fragment sequence number (1-based, incrementing).
/// * `decode_time` - Base media decode time for this fragment in timescale units.
/// * `samples` - Slice of sample information including raw data.
pub fn write_media_segment(seq: u32, decode_time: u64, samples: &[SampleInfo]) -> Vec<u8> {
    // Track ID is always 1 for single-track segments.
    let track_id = 1u32;

    let mfhd = boxes::write_mfhd(seq);
    let tfhd = boxes::write_tfhd(track_id);
    let tfdt = boxes::write_tfdt(decode_time);

    // Build trun samples
    let trun_samples: Vec<TrunSample> = samples
        .iter()
        .map(|s| TrunSample {
            size: s.data.len() as u32,
            flags: if s.is_keyframe {
                0x02000000
            } else {
                0x01010000
            },
            composition_time_offset: s.composition_offset,
        })
        .collect();

    // We need to compute the data offset, which is the distance from the
    // start of moof to the first byte of sample data in mdat.
    // data_offset = moof_size + mdat_header_size
    //
    // moof structure:
    //   moof header (8)
    //     mfhd (16)
    //     traf header (8)
    //       tfhd (16)
    //       tfdt (20)
    //       trun (8 + 4 + 4 + 4 + samples * 12)
    //
    // We need to know the total moof size first, then compute data_offset.

    // trun content size: fullbox_header(4) + sample_count(4) + data_offset(4) + samples * 12
    let trun_content_size = 4 + 4 + 4 + trun_samples.len() * 12;
    let trun_box_size = 8 + trun_content_size;

    // traf size: 8 (header) + tfhd + tfdt + trun
    let traf_size = 8 + tfhd.len() + tfdt.len() + trun_box_size;

    // moof size: 8 (header) + mfhd + traf
    let moof_size = 8 + mfhd.len() + traf_size;

    // Calculate total sample data size
    let total_data_size: u64 = samples.iter().map(|s| s.data.len() as u64).sum();

    // mdat header size
    let mdat_hdr_size: usize = if total_data_size + 8 > u32::MAX as u64 {
        16
    } else {
        8
    };

    let data_offset = (moof_size + mdat_hdr_size) as i32;

    // Now write the trun with the correct data_offset
    let trun = boxes::write_trun(&trun_samples, data_offset);

    // Build traf
    let traf = boxes::write_container_box(b"traf", &[&tfhd, &tfdt, &trun]);

    // Build moof
    let moof = boxes::write_container_box(b"moof", &[&mfhd, &traf]);

    // Build mdat
    let mdat_hdr = boxes::write_mdat_header(total_data_size);

    // Concatenate: moof + mdat_header + sample data
    let mut result = Vec::with_capacity(moof.len() + mdat_hdr.len() + total_data_size as usize);
    result.extend_from_slice(&moof);
    result.extend_from_slice(&mdat_hdr);
    for sample in samples {
        result.extend_from_slice(&sample.data);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_u32(data: &[u8], offset: usize) -> u32 {
        u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ])
    }

    #[test]
    fn test_init_segment_contains_ftyp_and_moov() {
        let config = TrackConfig {
            track_id: 1,
            timescale: 90000,
            codec: Codec::Avc,
            width: 1920,
            height: 1080,
            sample_rate: 0,
            channels: 0,
            codec_private: vec![],
        };

        let init = write_init_segment(&config);

        // Should start with ftyp
        assert_eq!(&init[4..8], b"ftyp");

        // Should contain moov
        let ftyp_size = read_u32(&init, 0) as usize;
        assert_eq!(&init[ftyp_size + 4..ftyp_size + 8], b"moov");

        // ftyp + moov should span entire init segment
        let moov_size = read_u32(&init, ftyp_size) as usize;
        assert_eq!(ftyp_size + moov_size, init.len());
    }

    #[test]
    fn test_init_segment_audio() {
        let config = TrackConfig {
            track_id: 1,
            timescale: 48000,
            codec: Codec::Aac,
            width: 0,
            height: 0,
            sample_rate: 48000,
            channels: 2,
            codec_private: vec![],
        };

        let init = write_init_segment(&config);
        assert_eq!(&init[4..8], b"ftyp");
        let ftyp_size = read_u32(&init, 0) as usize;
        assert_eq!(&init[ftyp_size + 4..ftyp_size + 8], b"moov");
    }

    #[test]
    fn test_media_segment_structure() {
        let samples = vec![
            SampleInfo {
                data: vec![0xAA; 100],
                duration: 3000,
                is_keyframe: true,
                composition_offset: 0,
            },
            SampleInfo {
                data: vec![0xBB; 50],
                duration: 3000,
                is_keyframe: false,
                composition_offset: 1500,
            },
        ];

        let segment = write_media_segment(1, 0, &samples);

        // Should start with moof
        assert_eq!(&segment[4..8], b"moof");

        // moof should be followed by mdat
        let moof_size = read_u32(&segment, 0) as usize;
        assert_eq!(&segment[moof_size + 4..moof_size + 8], b"mdat");

        // mdat should contain the sample data
        let mdat_size = read_u32(&segment, moof_size) as usize;
        assert_eq!(mdat_size, 8 + 150); // header + 100 + 50 bytes of data

        // Total segment size
        assert_eq!(segment.len(), moof_size + mdat_size);
    }

    #[test]
    fn test_media_segment_empty_samples() {
        let segment = write_media_segment(1, 0, &[]);
        // Should still produce valid moof + mdat
        assert_eq!(&segment[4..8], b"moof");
        let moof_size = read_u32(&segment, 0) as usize;
        assert_eq!(&segment[moof_size + 4..moof_size + 8], b"mdat");
    }

    #[test]
    fn test_media_segment_data_offset() {
        let samples = vec![SampleInfo {
            data: vec![0xFF; 200],
            duration: 1000,
            is_keyframe: true,
            composition_offset: 0,
        }];

        let segment = write_media_segment(1, 0, &samples);
        let moof_size = read_u32(&segment, 0) as usize;
        let mdat_hdr_size = 8usize; // normal mdat header

        // The data starts at moof_size + mdat_hdr_size
        let data_start = moof_size + mdat_hdr_size;
        assert_eq!(&segment[data_start..data_start + 200], &[0xFF; 200]);
    }

    #[test]
    fn test_box_sizes_are_consistent() {
        let config = TrackConfig {
            track_id: 1,
            timescale: 90000,
            codec: Codec::Avc,
            width: 1920,
            height: 1080,
            sample_rate: 0,
            channels: 0,
            codec_private: vec![0x01, 0x64, 0x00, 0x1F],
        };

        let init = write_init_segment(&config);

        // Walk all top-level boxes and verify sizes sum to total
        let mut pos = 0;
        let mut count = 0;
        while pos + 8 <= init.len() {
            let size = read_u32(&init, pos) as usize;
            assert!(size >= 8, "Box size too small at offset {}", pos);
            assert!(
                pos + size <= init.len(),
                "Box at offset {} extends beyond data",
                pos
            );
            pos += size;
            count += 1;
        }
        assert_eq!(pos, init.len(), "Boxes do not span entire init segment");
        assert_eq!(count, 2, "Expected exactly 2 top-level boxes (ftyp + moov)");
    }
}
