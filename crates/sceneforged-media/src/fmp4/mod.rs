//! Fragmented MP4 (fMP4) serialization.
//!
//! This module generates fMP4 structures for HLS serving:
//! - Init segment (ftyp + moov with track info)
//! - Segment moof boxes (fragment headers)

mod moof;

pub use moof::MoofBuilder;

use bytes::{BufMut, BytesMut};

/// Initialization segment containing ftyp and moov.
#[derive(Debug, Clone)]
pub struct InitSegment {
    /// Serialized init segment data.
    pub data: Vec<u8>,
    /// Track timescale.
    pub timescale: u32,
    /// Duration in timescale units.
    pub duration: u64,
}

impl InitSegment {
    /// Create a new init segment builder.
    pub fn builder() -> InitSegmentBuilder {
        InitSegmentBuilder::new()
    }
}

/// Builder for creating init segments.
pub struct InitSegmentBuilder {
    timescale: u32,
    duration: u64,
    width: u32,
    height: u32,
    video_codec: Option<Vec<u8>>, // avcC or hvcC
    has_audio: bool,
    audio_timescale: u32,
    audio_channels: u16,
    audio_sample_rate: u32,
    audio_codec: Option<Vec<u8>>, // esds
}

impl InitSegmentBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            timescale: 90000,
            duration: 0,
            width: 1920,
            height: 1080,
            video_codec: None,
            has_audio: false,
            audio_timescale: 48000,
            audio_channels: 2,
            audio_sample_rate: 48000,
            audio_codec: None,
        }
    }

    /// Set video timescale.
    pub fn timescale(mut self, ts: u32) -> Self {
        self.timescale = ts;
        self
    }

    /// Set duration.
    pub fn duration(mut self, d: u64) -> Self {
        self.duration = d;
        self
    }

    /// Set video dimensions.
    pub fn dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Set video codec configuration (avcC box contents).
    pub fn video_codec(mut self, data: Vec<u8>) -> Self {
        self.video_codec = Some(data);
        self
    }

    /// Add audio track.
    pub fn with_audio(mut self, timescale: u32, channels: u16, sample_rate: u32) -> Self {
        self.has_audio = true;
        self.audio_timescale = timescale;
        self.audio_channels = channels;
        self.audio_sample_rate = sample_rate;
        self
    }

    /// Build the init segment.
    pub fn build(self) -> InitSegment {
        let mut buf = BytesMut::with_capacity(1024);

        // ftyp box
        self.write_ftyp(&mut buf);

        // moov box
        self.write_moov(&mut buf);

        InitSegment {
            data: buf.to_vec(),
            timescale: self.timescale,
            duration: self.duration,
        }
    }

    fn write_ftyp(&self, buf: &mut BytesMut) {
        // ftyp: isom, iso5, dash, mp42
        let brands = [b"isom", b"iso5", b"dash", b"mp42"];
        let size = 8 + 4 + 4 + brands.len() * 4;

        buf.put_u32(size as u32);
        buf.put_slice(b"ftyp");
        buf.put_slice(b"isom"); // major brand
        buf.put_u32(0x200); // minor version
        for brand in &brands {
            buf.put_slice(*brand);
        }
    }

    fn write_moov(&self, buf: &mut BytesMut) {
        let moov_start = buf.len();
        buf.put_u32(0); // placeholder size
        buf.put_slice(b"moov");

        // mvhd
        self.write_mvhd(buf);

        // Video trak
        self.write_video_trak(buf, 1);

        // Audio trak if present
        if self.has_audio {
            self.write_audio_trak(buf, 2);
        }

        // mvex (movie extends for fragmented)
        self.write_mvex(buf);

        // Update moov size
        let moov_size = buf.len() - moov_start;
        let size_bytes = (moov_size as u32).to_be_bytes();
        buf[moov_start..moov_start + 4].copy_from_slice(&size_bytes);
    }

    fn write_mvhd(&self, buf: &mut BytesMut) {
        let size = 120; // version 1
        buf.put_u32(size);
        buf.put_slice(b"mvhd");
        buf.put_u8(1); // version 1
        buf.put_slice(&[0, 0, 0]); // flags
        buf.put_u64(0); // creation time
        buf.put_u64(0); // modification time
        buf.put_u32(self.timescale);
        buf.put_u64(self.duration);
        buf.put_u32(0x00010000); // rate = 1.0
        buf.put_u16(0x0100); // volume = 1.0
        buf.put_u16(0); // reserved
        buf.put_u64(0); // reserved
        // Matrix (identity)
        buf.put_u32(0x00010000);
        buf.put_u32(0);
        buf.put_u32(0);
        buf.put_u32(0);
        buf.put_u32(0x00010000);
        buf.put_u32(0);
        buf.put_u32(0);
        buf.put_u32(0);
        buf.put_u32(0x40000000);
        // Pre-defined (6 * 4 bytes)
        for _ in 0..6 {
            buf.put_u32(0);
        }
        buf.put_u32(3); // next track ID
    }

    fn write_video_trak(&self, buf: &mut BytesMut, track_id: u32) {
        let trak_start = buf.len();
        buf.put_u32(0);
        buf.put_slice(b"trak");

        // tkhd
        self.write_tkhd(buf, track_id, true);

        // mdia
        self.write_video_mdia(buf, track_id);

        let trak_size = buf.len() - trak_start;
        let size_bytes = (trak_size as u32).to_be_bytes();
        buf[trak_start..trak_start + 4].copy_from_slice(&size_bytes);
    }

    fn write_audio_trak(&self, buf: &mut BytesMut, track_id: u32) {
        let trak_start = buf.len();
        buf.put_u32(0);
        buf.put_slice(b"trak");

        // tkhd
        self.write_tkhd(buf, track_id, false);

        // mdia
        self.write_audio_mdia(buf, track_id);

        let trak_size = buf.len() - trak_start;
        let size_bytes = (trak_size as u32).to_be_bytes();
        buf[trak_start..trak_start + 4].copy_from_slice(&size_bytes);
    }

    fn write_tkhd(&self, buf: &mut BytesMut, track_id: u32, is_video: bool) {
        let size = 104; // version 1
        buf.put_u32(size);
        buf.put_slice(b"tkhd");
        buf.put_u8(1); // version 1
        buf.put_slice(&[0, 0, 7]); // flags: enabled, in_movie, in_preview
        buf.put_u64(0); // creation time
        buf.put_u64(0); // modification time
        buf.put_u32(track_id);
        buf.put_u32(0); // reserved
        buf.put_u64(self.duration);
        buf.put_u64(0); // reserved
        buf.put_u16(0); // layer
        buf.put_u16(0); // alternate group
        buf.put_u16(if is_video { 0 } else { 0x0100 }); // volume
        buf.put_u16(0); // reserved
        // Matrix (identity)
        buf.put_u32(0x00010000);
        buf.put_u32(0);
        buf.put_u32(0);
        buf.put_u32(0);
        buf.put_u32(0x00010000);
        buf.put_u32(0);
        buf.put_u32(0);
        buf.put_u32(0);
        buf.put_u32(0x40000000);
        // Width and height (16.16 fixed point)
        if is_video {
            buf.put_u32(self.width << 16);
            buf.put_u32(self.height << 16);
        } else {
            buf.put_u32(0);
            buf.put_u32(0);
        }
    }

    fn write_video_mdia(&self, buf: &mut BytesMut, _track_id: u32) {
        let mdia_start = buf.len();
        buf.put_u32(0);
        buf.put_slice(b"mdia");

        // mdhd
        self.write_mdhd(buf, self.timescale);

        // hdlr
        self.write_hdlr(buf, b"vide", b"VideoHandler");

        // minf
        self.write_video_minf(buf);

        let mdia_size = buf.len() - mdia_start;
        let size_bytes = (mdia_size as u32).to_be_bytes();
        buf[mdia_start..mdia_start + 4].copy_from_slice(&size_bytes);
    }

    fn write_audio_mdia(&self, buf: &mut BytesMut, _track_id: u32) {
        let mdia_start = buf.len();
        buf.put_u32(0);
        buf.put_slice(b"mdia");

        // mdhd
        self.write_mdhd(buf, self.audio_timescale);

        // hdlr
        self.write_hdlr(buf, b"soun", b"SoundHandler");

        // minf
        self.write_audio_minf(buf);

        let mdia_size = buf.len() - mdia_start;
        let size_bytes = (mdia_size as u32).to_be_bytes();
        buf[mdia_start..mdia_start + 4].copy_from_slice(&size_bytes);
    }

    fn write_mdhd(&self, buf: &mut BytesMut, timescale: u32) {
        let size = 44; // version 1
        buf.put_u32(size);
        buf.put_slice(b"mdhd");
        buf.put_u8(1); // version 1
        buf.put_slice(&[0, 0, 0]); // flags
        buf.put_u64(0); // creation time
        buf.put_u64(0); // modification time
        buf.put_u32(timescale);
        buf.put_u64(self.duration);
        buf.put_u16(0x55C4); // language: und
        buf.put_u16(0); // pre_defined
    }

    fn write_hdlr(&self, buf: &mut BytesMut, handler: &[u8; 4], name: &[u8]) {
        let size = 32 + name.len() + 1;
        buf.put_u32(size as u32);
        buf.put_slice(b"hdlr");
        buf.put_u32(0); // version/flags
        buf.put_u32(0); // pre_defined
        buf.put_slice(handler);
        buf.put_u32(0); // reserved
        buf.put_u32(0);
        buf.put_u32(0);
        buf.put_slice(name);
        buf.put_u8(0); // null terminator
    }

    fn write_video_minf(&self, buf: &mut BytesMut) {
        let minf_start = buf.len();
        buf.put_u32(0);
        buf.put_slice(b"minf");

        // vmhd
        buf.put_u32(20);
        buf.put_slice(b"vmhd");
        buf.put_u32(1); // version/flags
        buf.put_u16(0); // graphics mode
        buf.put_u16(0);
        buf.put_u16(0);
        buf.put_u16(0); // opcolor

        // dinf
        self.write_dinf(buf);

        // stbl
        self.write_video_stbl(buf);

        let minf_size = buf.len() - minf_start;
        let size_bytes = (minf_size as u32).to_be_bytes();
        buf[minf_start..minf_start + 4].copy_from_slice(&size_bytes);
    }

    fn write_audio_minf(&self, buf: &mut BytesMut) {
        let minf_start = buf.len();
        buf.put_u32(0);
        buf.put_slice(b"minf");

        // smhd
        buf.put_u32(16);
        buf.put_slice(b"smhd");
        buf.put_u32(0); // version/flags
        buf.put_u16(0); // balance
        buf.put_u16(0); // reserved

        // dinf
        self.write_dinf(buf);

        // stbl
        self.write_audio_stbl(buf);

        let minf_size = buf.len() - minf_start;
        let size_bytes = (minf_size as u32).to_be_bytes();
        buf[minf_start..minf_start + 4].copy_from_slice(&size_bytes);
    }

    fn write_dinf(&self, buf: &mut BytesMut) {
        buf.put_u32(36);
        buf.put_slice(b"dinf");

        // dref
        buf.put_u32(28);
        buf.put_slice(b"dref");
        buf.put_u32(0); // version/flags
        buf.put_u32(1); // entry count

        // url (self-reference)
        buf.put_u32(12);
        buf.put_slice(b"url ");
        buf.put_u32(1); // flags: self-contained
    }

    fn write_video_stbl(&self, buf: &mut BytesMut) {
        let stbl_start = buf.len();
        buf.put_u32(0);
        buf.put_slice(b"stbl");

        // stsd
        self.write_video_stsd(buf);

        // Empty sample tables (required for fMP4)
        self.write_empty_stts(buf);
        self.write_empty_stsc(buf);
        self.write_empty_stsz(buf);
        self.write_empty_stco(buf);

        let stbl_size = buf.len() - stbl_start;
        let size_bytes = (stbl_size as u32).to_be_bytes();
        buf[stbl_start..stbl_start + 4].copy_from_slice(&size_bytes);
    }

    fn write_audio_stbl(&self, buf: &mut BytesMut) {
        let stbl_start = buf.len();
        buf.put_u32(0);
        buf.put_slice(b"stbl");

        // stsd
        self.write_audio_stsd(buf);

        // Empty sample tables
        self.write_empty_stts(buf);
        self.write_empty_stsc(buf);
        self.write_empty_stsz(buf);
        self.write_empty_stco(buf);

        let stbl_size = buf.len() - stbl_start;
        let size_bytes = (stbl_size as u32).to_be_bytes();
        buf[stbl_start..stbl_start + 4].copy_from_slice(&size_bytes);
    }

    fn write_video_stsd(&self, buf: &mut BytesMut) {
        let stsd_start = buf.len();
        buf.put_u32(0);
        buf.put_slice(b"stsd");
        buf.put_u32(0); // version/flags
        buf.put_u32(1); // entry count

        // avc1 sample entry
        let avc1_start = buf.len();
        buf.put_u32(0);
        buf.put_slice(b"avc1");
        buf.put_slice(&[0; 6]); // reserved
        buf.put_u16(1); // data reference index
        buf.put_u16(0); // pre_defined
        buf.put_u16(0); // reserved
        buf.put_slice(&[0; 12]); // pre_defined
        buf.put_u16(self.width as u16);
        buf.put_u16(self.height as u16);
        buf.put_u32(0x00480000); // horiz resolution 72 dpi
        buf.put_u32(0x00480000); // vert resolution 72 dpi
        buf.put_u32(0); // reserved
        buf.put_u16(1); // frame count
        buf.put_slice(&[0; 32]); // compressor name
        buf.put_u16(0x0018); // depth
        buf.put_i16(-1); // pre_defined

        // avcC if available
        if let Some(ref avcc) = self.video_codec {
            buf.put_u32((8 + avcc.len()) as u32);
            buf.put_slice(b"avcC");
            buf.put_slice(avcc);
        }

        let avc1_size = buf.len() - avc1_start;
        let size_bytes = (avc1_size as u32).to_be_bytes();
        buf[avc1_start..avc1_start + 4].copy_from_slice(&size_bytes);

        let stsd_size = buf.len() - stsd_start;
        let size_bytes = (stsd_size as u32).to_be_bytes();
        buf[stsd_start..stsd_start + 4].copy_from_slice(&size_bytes);
    }

    fn write_audio_stsd(&self, buf: &mut BytesMut) {
        let stsd_start = buf.len();
        buf.put_u32(0);
        buf.put_slice(b"stsd");
        buf.put_u32(0); // version/flags
        buf.put_u32(1); // entry count

        // mp4a sample entry
        let mp4a_start = buf.len();
        buf.put_u32(0);
        buf.put_slice(b"mp4a");
        buf.put_slice(&[0; 6]); // reserved
        buf.put_u16(1); // data reference index
        buf.put_u32(0); // reserved
        buf.put_u32(0); // reserved
        buf.put_u16(self.audio_channels);
        buf.put_u16(16); // sample size
        buf.put_u16(0); // pre_defined
        buf.put_u16(0); // reserved
        buf.put_u32(self.audio_sample_rate << 16);

        // esds if available
        if let Some(ref esds) = self.audio_codec {
            buf.put_u32((8 + esds.len()) as u32);
            buf.put_slice(b"esds");
            buf.put_slice(esds);
        }

        let mp4a_size = buf.len() - mp4a_start;
        let size_bytes = (mp4a_size as u32).to_be_bytes();
        buf[mp4a_start..mp4a_start + 4].copy_from_slice(&size_bytes);

        let stsd_size = buf.len() - stsd_start;
        let size_bytes = (stsd_size as u32).to_be_bytes();
        buf[stsd_start..stsd_start + 4].copy_from_slice(&size_bytes);
    }

    fn write_empty_stts(&self, buf: &mut BytesMut) {
        buf.put_u32(16);
        buf.put_slice(b"stts");
        buf.put_u32(0); // version/flags
        buf.put_u32(0); // entry count
    }

    fn write_empty_stsc(&self, buf: &mut BytesMut) {
        buf.put_u32(16);
        buf.put_slice(b"stsc");
        buf.put_u32(0); // version/flags
        buf.put_u32(0); // entry count
    }

    fn write_empty_stsz(&self, buf: &mut BytesMut) {
        buf.put_u32(20);
        buf.put_slice(b"stsz");
        buf.put_u32(0); // version/flags
        buf.put_u32(0); // sample size
        buf.put_u32(0); // sample count
    }

    fn write_empty_stco(&self, buf: &mut BytesMut) {
        buf.put_u32(16);
        buf.put_slice(b"stco");
        buf.put_u32(0); // version/flags
        buf.put_u32(0); // entry count
    }

    fn write_mvex(&self, buf: &mut BytesMut) {
        let mvex_start = buf.len();
        buf.put_u32(0);
        buf.put_slice(b"mvex");

        // trex for video
        self.write_trex(buf, 1);

        // trex for audio if present
        if self.has_audio {
            self.write_trex(buf, 2);
        }

        let mvex_size = buf.len() - mvex_start;
        let size_bytes = (mvex_size as u32).to_be_bytes();
        buf[mvex_start..mvex_start + 4].copy_from_slice(&size_bytes);
    }

    fn write_trex(&self, buf: &mut BytesMut, track_id: u32) {
        buf.put_u32(32);
        buf.put_slice(b"trex");
        buf.put_u32(0); // version/flags
        buf.put_u32(track_id);
        buf.put_u32(1); // default sample description index
        buf.put_u32(0); // default sample duration
        buf.put_u32(0); // default sample size
        buf.put_u32(0); // default sample flags
    }
}

impl Default for InitSegmentBuilder {
    fn default() -> Self {
        Self::new()
    }
}
