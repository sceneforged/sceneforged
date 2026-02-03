//! Movie fragment (moof) box builder.

use crate::mp4::SampleEntry;
use bytes::{BufMut, BytesMut};

/// A track's fragment data for multi-track moof building.
pub struct TrackFragment<'a> {
    /// Track ID (1-based).
    pub track_id: u32,
    /// Base media decode time for this track's fragment.
    pub base_media_decode_time: u64,
    /// Samples in this fragment.
    pub samples: &'a [SampleEntry],
}

/// Build a moof+mdat header for multiple tracks (e.g. video + audio).
///
/// The mdat data layout is: track1 data, then track2 data, etc.
/// Each trun's data_offset points to where its track's data starts in the mdat.
/// The caller must stream byte ranges in the same order (track1 ranges, then track2 ranges).
pub fn build_multi_track_moof(sequence_number: u32, tracks: &[TrackFragment<'_>]) -> Vec<u8> {
    let total_samples: usize = tracks.iter().map(|t| t.samples.len()).sum();
    let mut buf = BytesMut::with_capacity(256 + total_samples * 16);

    let moof_start = buf.len();
    buf.put_u32(0); // placeholder moof size
    buf.put_slice(b"moof");

    // mfhd
    buf.put_u32(16);
    buf.put_slice(b"mfhd");
    buf.put_u32(0); // version/flags
    buf.put_u32(sequence_number);

    // Write all trafs, collecting data_offset placeholder positions
    let mut data_offset_positions: Vec<usize> = Vec::with_capacity(tracks.len());

    for track in tracks {
        let traf_start = buf.len();
        buf.put_u32(0); // placeholder traf size
        buf.put_slice(b"traf");

        // tfhd
        buf.put_u32(16);
        buf.put_slice(b"tfhd");
        buf.put_u32(0x020000); // default-base-is-moof
        buf.put_u32(track.track_id);

        // tfdt
        buf.put_u32(20);
        buf.put_slice(b"tfdt");
        buf.put_u32(0x01000000); // version 1
        buf.put_u64(track.base_media_decode_time);

        // trun
        let flags: u32 = 0x000001 | 0x000200 | 0x000400 | 0x000800;
        let trun_size = 12 + 4 + 4 + track.samples.len() * 12;
        buf.put_u32(trun_size as u32);
        buf.put_slice(b"trun");
        buf.put_u32(0x01000000 | flags); // version 1
        buf.put_u32(track.samples.len() as u32);

        // data_offset placeholder
        data_offset_positions.push(buf.len());
        buf.put_u32(0);

        for sample in track.samples {
            buf.put_u32(sample.size);
            let sample_flags = if sample.is_keyframe {
                0x02000000u32
            } else {
                0x01010000u32
            };
            buf.put_u32(sample_flags);
            buf.put_i32(sample.cts_offset);
        }

        // Update traf size
        let traf_size = buf.len() - traf_start;
        let size_bytes = (traf_size as u32).to_be_bytes();
        buf[traf_start..traf_start + 4].copy_from_slice(&size_bytes);
    }

    // Update moof size
    let moof_size = buf.len() - moof_start;
    let size_bytes = (moof_size as u32).to_be_bytes();
    buf[moof_start..moof_start + 4].copy_from_slice(&size_bytes);

    // Calculate total data size across all tracks
    let total_data_size: u64 = tracks
        .iter()
        .flat_map(|t| t.samples.iter())
        .map(|s| s.size as u64)
        .sum();

    // Determine mdat header size
    let mdat_header_size: u64 = if total_data_size + 8 > u32::MAX as u64 {
        16
    } else {
        8
    };

    // Fix up data_offsets: each track's data starts after all previous tracks' data
    let mut data_start = moof_size as u64 + mdat_header_size;
    for (i, track) in tracks.iter().enumerate() {
        let offset_bytes = (data_start as i32).to_be_bytes();
        buf[data_offset_positions[i]..data_offset_positions[i] + 4]
            .copy_from_slice(&offset_bytes);
        let track_data_size: u64 = track.samples.iter().map(|s| s.size as u64).sum();
        data_start += track_data_size;
    }

    // Write mdat header
    if total_data_size + 8 > u32::MAX as u64 {
        buf.put_u32(1);
        buf.put_slice(b"mdat");
        buf.put_u64(total_data_size + 16);
    } else {
        buf.put_u32((total_data_size + 8) as u32);
        buf.put_slice(b"mdat");
    }

    buf.to_vec()
}

/// Builder for creating moof boxes for HLS segments.
pub struct MoofBuilder {
    sequence_number: u32,
    track_id: u32,
    base_media_decode_time: u64,
}

impl MoofBuilder {
    /// Create a new moof builder.
    pub fn new(sequence_number: u32, track_id: u32) -> Self {
        Self {
            sequence_number,
            track_id,
            base_media_decode_time: 0,
        }
    }

    /// Set base media decode time.
    pub fn base_media_decode_time(mut self, time: u64) -> Self {
        self.base_media_decode_time = time;
        self
    }

    /// Build moof + mdat header for the given samples.
    ///
    /// Returns the serialized moof box followed by an 8-byte mdat header.
    /// The actual sample data should be appended after this.
    pub fn build(self, samples: &[SampleEntry]) -> Vec<u8> {
        let mut buf = BytesMut::with_capacity(256 + samples.len() * 16);

        self.write_moof(&mut buf, samples);

        // Write mdat header
        let data_size: u64 = samples.iter().map(|s| s.size as u64).sum();
        self.write_mdat_header(&mut buf, data_size);

        buf.to_vec()
    }

    /// Build just the moof box without mdat.
    pub fn build_moof_only(self, samples: &[SampleEntry]) -> Vec<u8> {
        let mut buf = BytesMut::with_capacity(256 + samples.len() * 16);
        self.write_moof(&mut buf, samples);
        buf.to_vec()
    }

    fn write_moof(&self, buf: &mut BytesMut, samples: &[SampleEntry]) {
        let moof_start = buf.len();
        buf.put_u32(0); // placeholder
        buf.put_slice(b"moof");

        // mfhd (movie fragment header)
        self.write_mfhd(buf);

        // traf (track fragment)
        self.write_traf(buf, samples);

        // Update moof size
        let moof_size = buf.len() - moof_start;
        let size_bytes = (moof_size as u32).to_be_bytes();
        buf[moof_start..moof_start + 4].copy_from_slice(&size_bytes);
    }

    fn write_mfhd(&self, buf: &mut BytesMut) {
        buf.put_u32(16);
        buf.put_slice(b"mfhd");
        buf.put_u32(0); // version/flags
        buf.put_u32(self.sequence_number);
    }

    fn write_traf(&self, buf: &mut BytesMut, samples: &[SampleEntry]) {
        let traf_start = buf.len();
        buf.put_u32(0); // placeholder
        buf.put_slice(b"traf");

        // tfhd (track fragment header)
        self.write_tfhd(buf);

        // tfdt (track fragment decode time)
        self.write_tfdt(buf);

        // trun (track run)
        self.write_trun(buf, samples, traf_start);

        // Update traf size
        let traf_size = buf.len() - traf_start;
        let size_bytes = (traf_size as u32).to_be_bytes();
        buf[traf_start..traf_start + 4].copy_from_slice(&size_bytes);
    }

    fn write_tfhd(&self, buf: &mut BytesMut) {
        // Flags: default-base-is-moof (0x020000)
        buf.put_u32(16);
        buf.put_slice(b"tfhd");
        buf.put_u32(0x020000); // version 0, flags
        buf.put_u32(self.track_id);
    }

    fn write_tfdt(&self, buf: &mut BytesMut) {
        // Version 1 for 64-bit decode time
        buf.put_u32(20);
        buf.put_slice(b"tfdt");
        buf.put_u32(0x01000000); // version 1
        buf.put_u64(self.base_media_decode_time);
    }

    fn write_trun(&self, buf: &mut BytesMut, samples: &[SampleEntry], _traf_start: usize) {
        // Flags:
        // 0x000001: data-offset-present
        // 0x000200: sample-size-present
        // 0x000400: sample-flags-present
        // 0x000800: sample-composition-time-offset-present
        let flags = 0x000001 | 0x000200 | 0x000400 | 0x000800;

        let header_size = 12 + 4 + 4 + samples.len() * 12; // box(8) + ver/flags(4) + count(4) + data_offset(4) + samples * (size(4) + flags(4) + cts_offset(4))

        buf.put_u32(header_size as u32);
        buf.put_slice(b"trun");
        buf.put_u32(0x01000000 | flags); // version 1 (signed CTS offsets)
        buf.put_u32(samples.len() as u32);

        // Data offset: offset from start of moof to start of mdat data
        // We'll write a placeholder and fix it up later
        let data_offset_pos = buf.len();
        buf.put_u32(0); // placeholder

        // Sample entries
        for sample in samples {
            buf.put_u32(sample.size);

            // Sample flags: sync sample = 0x02000000, non-sync = 0x01010000
            let sample_flags = if sample.is_keyframe {
                0x02000000u32
            } else {
                0x01010000u32
            };
            buf.put_u32(sample_flags);

            // Composition time offset (signed in version 1)
            buf.put_i32(sample.cts_offset);
        }

        // Calculate and write data offset
        // With default-base-is-moof flag, data_offset is relative to moof start (which is 0)
        // data_offset = moof_size + mdat_header_size (8 bytes for regular mdat)
        let data_offset = (buf.len() + 8) as i32;
        let offset_bytes = data_offset.to_be_bytes();
        buf[data_offset_pos..data_offset_pos + 4].copy_from_slice(&offset_bytes);
    }

    fn write_mdat_header(&self, buf: &mut BytesMut, data_size: u64) {
        if data_size + 8 > u32::MAX as u64 {
            // Extended size
            buf.put_u32(1);
            buf.put_slice(b"mdat");
            buf.put_u64(data_size + 16);
        } else {
            buf.put_u32((data_size + 8) as u32);
            buf.put_slice(b"mdat");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moof_builder() {
        let samples = vec![
            SampleEntry {
                index: 0,
                offset: 1000,
                size: 1000,
                dts: 0,
                cts_offset: 0,
                is_keyframe: true,
            },
            SampleEntry {
                index: 1,
                offset: 2000,
                size: 500,
                dts: 1000,
                cts_offset: 500,
                is_keyframe: false,
            },
        ];

        let moof_data = MoofBuilder::new(1, 1)
            .base_media_decode_time(0)
            .build(&samples);

        // Check moof header
        assert_eq!(&moof_data[4..8], b"moof");

        // Check mdat header is present at the end
        assert!(moof_data.len() > 8);
        // mdat header should be at the end (after moof box)
        let mdat_header_start = moof_data.len() - 8;
        assert_eq!(
            &moof_data[mdat_header_start + 4..mdat_header_start + 8],
            b"mdat"
        );
    }

    #[test]
    fn test_moof_only() {
        let samples = vec![SampleEntry {
            index: 0,
            offset: 0,
            size: 100,
            dts: 0,
            cts_offset: 0,
            is_keyframe: true,
        }];

        let moof_data = MoofBuilder::new(1, 1).build_moof_only(&samples);

        // Should not have mdat header
        assert!(!moof_data.ends_with(b"mdat"));
    }
}
