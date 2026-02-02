//! Movie fragment (moof) box builder.

use crate::mp4::SampleEntry;
use bytes::{BufMut, BytesMut};

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

        let header_size = 12 + 4 + samples.len() * 12; // Each sample: size(4) + flags(4) + cts_offset(4)

        buf.put_u32(header_size as u32);
        buf.put_slice(b"trun");
        buf.put_u32(flags); // version 0
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
