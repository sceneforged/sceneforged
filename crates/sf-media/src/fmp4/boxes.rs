//! ISO BMFF box types and serialization primitives.
//!
//! Each box follows the standard layout: 4-byte size (big-endian u32),
//! 4-byte type (ASCII), then box-specific content.

use serde::{Deserialize, Serialize};

/// Codec type for a track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Codec {
    /// H.264 / AVC video codec.
    Avc,
    /// H.265 / HEVC video codec.
    Hevc,
    /// AAC audio codec.
    Aac,
}

// ---------------------------------------------------------------------------
// Low-level box writing helpers
// ---------------------------------------------------------------------------

/// Write a complete box: size (u32 BE) + type (4 ASCII bytes) + content.
/// Returns the bytes.
pub(crate) fn write_box(box_type: &[u8; 4], content: &[u8]) -> Vec<u8> {
    let size = (8 + content.len()) as u32;
    let mut out = Vec::with_capacity(size as usize);
    out.extend_from_slice(&size.to_be_bytes());
    out.extend_from_slice(box_type);
    out.extend_from_slice(content);
    out
}

/// Write a container box (size + type + children concatenated).
pub(crate) fn write_container_box(box_type: &[u8; 4], children: &[&[u8]]) -> Vec<u8> {
    let children_len: usize = children.iter().map(|c| c.len()).sum();
    let size = (8 + children_len) as u32;
    let mut out = Vec::with_capacity(size as usize);
    out.extend_from_slice(&size.to_be_bytes());
    out.extend_from_slice(box_type);
    for child in children {
        out.extend_from_slice(child);
    }
    out
}

/// Write a full box header (version + flags) and return just that header portion.
pub(crate) fn fullbox_header(version: u8, flags: u32) -> [u8; 4] {
    let val = ((version as u32) << 24) | (flags & 0x00FFFFFF);
    val.to_be_bytes()
}

// ---------------------------------------------------------------------------
// ftyp box
// ---------------------------------------------------------------------------

/// Generate the `ftyp` box.
/// Major brand: "isom", minor version: 0x200,
/// Compatible brands: ["isom", "iso6", "mp41"].
pub(crate) fn write_ftyp() -> Vec<u8> {
    let mut content = Vec::with_capacity(4 + 4 + 3 * 4);
    // Major brand
    content.extend_from_slice(b"isom");
    // Minor version
    content.extend_from_slice(&0x200u32.to_be_bytes());
    // Compatible brands
    content.extend_from_slice(b"isom");
    content.extend_from_slice(b"iso6");
    content.extend_from_slice(b"mp41");
    write_box(b"ftyp", &content)
}

// ---------------------------------------------------------------------------
// mvhd box (movie header, version 1 for 64-bit times)
// ---------------------------------------------------------------------------

pub(crate) fn write_mvhd(timescale: u32, duration: u64) -> Vec<u8> {
    let mut content = Vec::with_capacity(112);
    // version 1, flags 0
    content.extend_from_slice(&fullbox_header(1, 0));
    // creation_time (u64)
    content.extend_from_slice(&0u64.to_be_bytes());
    // modification_time (u64)
    content.extend_from_slice(&0u64.to_be_bytes());
    // timescale (u32)
    content.extend_from_slice(&timescale.to_be_bytes());
    // duration (u64)
    content.extend_from_slice(&duration.to_be_bytes());
    // rate = 1.0 (fixed 16.16)
    content.extend_from_slice(&0x00010000u32.to_be_bytes());
    // volume = 1.0 (fixed 8.8)
    content.extend_from_slice(&0x0100u16.to_be_bytes());
    // reserved (2 + 8 bytes)
    content.extend_from_slice(&[0u8; 10]);
    // Matrix (identity 3x3, each 4 bytes, 36 bytes total)
    content.extend_from_slice(&0x00010000u32.to_be_bytes());
    content.extend_from_slice(&0u32.to_be_bytes());
    content.extend_from_slice(&0u32.to_be_bytes());
    content.extend_from_slice(&0u32.to_be_bytes());
    content.extend_from_slice(&0x00010000u32.to_be_bytes());
    content.extend_from_slice(&0u32.to_be_bytes());
    content.extend_from_slice(&0u32.to_be_bytes());
    content.extend_from_slice(&0u32.to_be_bytes());
    content.extend_from_slice(&0x40000000u32.to_be_bytes());
    // Pre-defined (6 * 4 = 24 bytes)
    content.extend_from_slice(&[0u8; 24]);
    // Next track ID
    content.extend_from_slice(&2u32.to_be_bytes());

    write_box(b"mvhd", &content)
}

// ---------------------------------------------------------------------------
// tkhd box (track header, version 1)
// ---------------------------------------------------------------------------

pub(crate) fn write_tkhd(
    track_id: u32,
    duration: u64,
    is_video: bool,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let mut content = Vec::with_capacity(96);
    // version 1, flags = 7 (enabled | in_movie | in_preview)
    content.extend_from_slice(&fullbox_header(1, 7));
    // creation_time (u64)
    content.extend_from_slice(&0u64.to_be_bytes());
    // modification_time (u64)
    content.extend_from_slice(&0u64.to_be_bytes());
    // track_id (u32)
    content.extend_from_slice(&track_id.to_be_bytes());
    // reserved (u32)
    content.extend_from_slice(&0u32.to_be_bytes());
    // duration (u64)
    content.extend_from_slice(&duration.to_be_bytes());
    // reserved (2 * u32)
    content.extend_from_slice(&[0u8; 8]);
    // layer (i16)
    content.extend_from_slice(&0u16.to_be_bytes());
    // alternate_group (i16)
    content.extend_from_slice(&0u16.to_be_bytes());
    // volume: 0x0100 for audio, 0 for video
    let volume: u16 = if is_video { 0 } else { 0x0100 };
    content.extend_from_slice(&volume.to_be_bytes());
    // reserved (u16)
    content.extend_from_slice(&0u16.to_be_bytes());
    // Matrix (identity)
    content.extend_from_slice(&0x00010000u32.to_be_bytes());
    content.extend_from_slice(&0u32.to_be_bytes());
    content.extend_from_slice(&0u32.to_be_bytes());
    content.extend_from_slice(&0u32.to_be_bytes());
    content.extend_from_slice(&0x00010000u32.to_be_bytes());
    content.extend_from_slice(&0u32.to_be_bytes());
    content.extend_from_slice(&0u32.to_be_bytes());
    content.extend_from_slice(&0u32.to_be_bytes());
    content.extend_from_slice(&0x40000000u32.to_be_bytes());
    // Width and height (16.16 fixed point)
    if is_video {
        content.extend_from_slice(&(width << 16).to_be_bytes());
        content.extend_from_slice(&(height << 16).to_be_bytes());
    } else {
        content.extend_from_slice(&0u32.to_be_bytes());
        content.extend_from_slice(&0u32.to_be_bytes());
    }

    write_box(b"tkhd", &content)
}

// ---------------------------------------------------------------------------
// mdhd box (media header, version 1)
// ---------------------------------------------------------------------------

pub(crate) fn write_mdhd(timescale: u32, duration: u64) -> Vec<u8> {
    let mut content = Vec::with_capacity(36);
    // version 1, flags 0
    content.extend_from_slice(&fullbox_header(1, 0));
    // creation_time (u64)
    content.extend_from_slice(&0u64.to_be_bytes());
    // modification_time (u64)
    content.extend_from_slice(&0u64.to_be_bytes());
    // timescale (u32)
    content.extend_from_slice(&timescale.to_be_bytes());
    // duration (u64)
    content.extend_from_slice(&duration.to_be_bytes());
    // language: undetermined (0x55C4)
    content.extend_from_slice(&0x55C4u16.to_be_bytes());
    // pre_defined
    content.extend_from_slice(&0u16.to_be_bytes());

    write_box(b"mdhd", &content)
}

// ---------------------------------------------------------------------------
// hdlr box (handler reference)
// ---------------------------------------------------------------------------

pub(crate) fn write_hdlr(handler_type: &[u8; 4], name: &[u8]) -> Vec<u8> {
    let mut content = Vec::with_capacity(24 + name.len() + 1);
    // version 0, flags 0
    content.extend_from_slice(&fullbox_header(0, 0));
    // pre_defined
    content.extend_from_slice(&0u32.to_be_bytes());
    // handler_type
    content.extend_from_slice(handler_type);
    // reserved (3 * u32)
    content.extend_from_slice(&[0u8; 12]);
    // name (null-terminated)
    content.extend_from_slice(name);
    content.push(0);

    write_box(b"hdlr", &content)
}

// ---------------------------------------------------------------------------
// dinf + dref boxes (data information)
// ---------------------------------------------------------------------------

pub(crate) fn write_dinf() -> Vec<u8> {
    // url box (self-contained)
    let url_box = {
        let mut c = Vec::with_capacity(4);
        c.extend_from_slice(&fullbox_header(0, 1)); // flags = 1 => self-contained
        write_box(b"url ", &c)
    };
    // dref box
    let dref_box = {
        let mut c = Vec::with_capacity(8 + url_box.len());
        c.extend_from_slice(&fullbox_header(0, 0));
        c.extend_from_slice(&1u32.to_be_bytes()); // entry count
        c.extend_from_slice(&url_box);
        write_box(b"dref", &c)
    };
    write_container_box(b"dinf", &[&dref_box])
}

// ---------------------------------------------------------------------------
// vmhd / smhd boxes (media information headers)
// ---------------------------------------------------------------------------

pub(crate) fn write_vmhd() -> Vec<u8> {
    let mut content = Vec::with_capacity(12);
    // version 0, flags 1
    content.extend_from_slice(&fullbox_header(0, 1));
    // graphicsmode
    content.extend_from_slice(&0u16.to_be_bytes());
    // opcolor (3 * u16)
    content.extend_from_slice(&[0u8; 6]);
    write_box(b"vmhd", &content)
}

pub(crate) fn write_smhd() -> Vec<u8> {
    let mut content = Vec::with_capacity(8);
    // version 0, flags 0
    content.extend_from_slice(&fullbox_header(0, 0));
    // balance
    content.extend_from_slice(&0u16.to_be_bytes());
    // reserved
    content.extend_from_slice(&0u16.to_be_bytes());
    write_box(b"smhd", &content)
}

// ---------------------------------------------------------------------------
// stbl and its required empty tables (for fMP4 init segments)
// ---------------------------------------------------------------------------

fn write_empty_stts() -> Vec<u8> {
    let mut content = Vec::with_capacity(8);
    content.extend_from_slice(&fullbox_header(0, 0));
    content.extend_from_slice(&0u32.to_be_bytes()); // entry count
    write_box(b"stts", &content)
}

fn write_empty_stsc() -> Vec<u8> {
    let mut content = Vec::with_capacity(8);
    content.extend_from_slice(&fullbox_header(0, 0));
    content.extend_from_slice(&0u32.to_be_bytes()); // entry count
    write_box(b"stsc", &content)
}

fn write_empty_stsz() -> Vec<u8> {
    let mut content = Vec::with_capacity(12);
    content.extend_from_slice(&fullbox_header(0, 0));
    content.extend_from_slice(&0u32.to_be_bytes()); // sample size
    content.extend_from_slice(&0u32.to_be_bytes()); // sample count
    write_box(b"stsz", &content)
}

fn write_empty_stco() -> Vec<u8> {
    let mut content = Vec::with_capacity(8);
    content.extend_from_slice(&fullbox_header(0, 0));
    content.extend_from_slice(&0u32.to_be_bytes()); // entry count
    write_box(b"stco", &content)
}

// ---------------------------------------------------------------------------
// stsd box (sample description) for video and audio
// ---------------------------------------------------------------------------

pub(crate) fn write_video_stsd(
    codec: &Codec,
    width: u32,
    height: u32,
    codec_private: &[u8],
) -> Vec<u8> {
    let sample_entry_type: &[u8; 4] = match codec {
        Codec::Avc => b"avc1",
        Codec::Hevc => b"hvc1",
        Codec::Aac => b"mp4a", // shouldn't happen for video
    };

    let codec_config_type: &[u8; 4] = match codec {
        Codec::Avc => b"avcC",
        Codec::Hevc => b"hvcC",
        Codec::Aac => b"esds",
    };

    // Visual sample entry content
    let mut entry = Vec::with_capacity(78 + codec_private.len() + 8);
    // reserved (6 bytes)
    entry.extend_from_slice(&[0u8; 6]);
    // data reference index
    entry.extend_from_slice(&1u16.to_be_bytes());
    // pre_defined + reserved (2 + 2 + 12 = 16 bytes)
    entry.extend_from_slice(&[0u8; 16]);
    // width (u16)
    entry.extend_from_slice(&(width as u16).to_be_bytes());
    // height (u16)
    entry.extend_from_slice(&(height as u16).to_be_bytes());
    // horiz resolution 72 dpi (fixed 16.16)
    entry.extend_from_slice(&0x00480000u32.to_be_bytes());
    // vert resolution 72 dpi
    entry.extend_from_slice(&0x00480000u32.to_be_bytes());
    // reserved (u32)
    entry.extend_from_slice(&0u32.to_be_bytes());
    // frame count
    entry.extend_from_slice(&1u16.to_be_bytes());
    // compressor name (32 bytes, null-padded)
    entry.extend_from_slice(&[0u8; 32]);
    // depth
    entry.extend_from_slice(&0x0018u16.to_be_bytes());
    // pre_defined (-1 as i16)
    entry.extend_from_slice(&(-1i16).to_be_bytes());

    // Codec configuration box (avcC or hvcC)
    if !codec_private.is_empty() {
        let config_box = write_box(codec_config_type, codec_private);
        entry.extend_from_slice(&config_box);
    }

    let sample_entry_box = write_box(sample_entry_type, &entry);

    // stsd
    let mut stsd_content = Vec::with_capacity(8 + sample_entry_box.len());
    stsd_content.extend_from_slice(&fullbox_header(0, 0));
    stsd_content.extend_from_slice(&1u32.to_be_bytes()); // entry count
    stsd_content.extend_from_slice(&sample_entry_box);

    write_box(b"stsd", &stsd_content)
}

pub(crate) fn write_audio_stsd(
    sample_rate: u32,
    channels: u16,
    codec_private: &[u8],
) -> Vec<u8> {
    // mp4a sample entry
    let mut entry = Vec::with_capacity(28 + codec_private.len() + 8);
    // reserved (6 bytes)
    entry.extend_from_slice(&[0u8; 6]);
    // data reference index
    entry.extend_from_slice(&1u16.to_be_bytes());
    // reserved (2 * u32)
    entry.extend_from_slice(&[0u8; 8]);
    // channel count
    entry.extend_from_slice(&channels.to_be_bytes());
    // sample size (16 bits)
    entry.extend_from_slice(&16u16.to_be_bytes());
    // pre_defined
    entry.extend_from_slice(&0u16.to_be_bytes());
    // reserved
    entry.extend_from_slice(&0u16.to_be_bytes());
    // sample rate (fixed 16.16)
    entry.extend_from_slice(&(sample_rate << 16).to_be_bytes());

    // esds box
    if !codec_private.is_empty() {
        let esds_box = write_box(b"esds", codec_private);
        entry.extend_from_slice(&esds_box);
    }

    let sample_entry_box = write_box(b"mp4a", &entry);

    // stsd
    let mut stsd_content = Vec::with_capacity(8 + sample_entry_box.len());
    stsd_content.extend_from_slice(&fullbox_header(0, 0));
    stsd_content.extend_from_slice(&1u32.to_be_bytes()); // entry count
    stsd_content.extend_from_slice(&sample_entry_box);

    write_box(b"stsd", &stsd_content)
}

// ---------------------------------------------------------------------------
// stbl (sample table) container for init segment
// ---------------------------------------------------------------------------

pub(crate) fn write_video_stbl(
    codec: &Codec,
    width: u32,
    height: u32,
    codec_private: &[u8],
) -> Vec<u8> {
    let stsd = write_video_stsd(codec, width, height, codec_private);
    let stts = write_empty_stts();
    let stsc = write_empty_stsc();
    let stsz = write_empty_stsz();
    let stco = write_empty_stco();
    write_container_box(b"stbl", &[&stsd, &stts, &stsc, &stsz, &stco])
}

pub(crate) fn write_audio_stbl(
    sample_rate: u32,
    channels: u16,
    codec_private: &[u8],
) -> Vec<u8> {
    let stsd = write_audio_stsd(sample_rate, channels, codec_private);
    let stts = write_empty_stts();
    let stsc = write_empty_stsc();
    let stsz = write_empty_stsz();
    let stco = write_empty_stco();
    write_container_box(b"stbl", &[&stsd, &stts, &stsc, &stsz, &stco])
}

// ---------------------------------------------------------------------------
// minf (media information) container
// ---------------------------------------------------------------------------

pub(crate) fn write_video_minf(
    codec: &Codec,
    width: u32,
    height: u32,
    codec_private: &[u8],
) -> Vec<u8> {
    let vmhd = write_vmhd();
    let dinf = write_dinf();
    let stbl = write_video_stbl(codec, width, height, codec_private);
    write_container_box(b"minf", &[&vmhd, &dinf, &stbl])
}

pub(crate) fn write_audio_minf(
    sample_rate: u32,
    channels: u16,
    codec_private: &[u8],
) -> Vec<u8> {
    let smhd = write_smhd();
    let dinf = write_dinf();
    let stbl = write_audio_stbl(sample_rate, channels, codec_private);
    write_container_box(b"minf", &[&smhd, &dinf, &stbl])
}

// ---------------------------------------------------------------------------
// mdia (media) container
// ---------------------------------------------------------------------------

pub(crate) fn write_video_mdia(
    timescale: u32,
    duration: u64,
    codec: &Codec,
    width: u32,
    height: u32,
    codec_private: &[u8],
) -> Vec<u8> {
    let mdhd = write_mdhd(timescale, duration);
    let hdlr = write_hdlr(b"vide", b"VideoHandler");
    let minf = write_video_minf(codec, width, height, codec_private);
    write_container_box(b"mdia", &[&mdhd, &hdlr, &minf])
}

pub(crate) fn write_audio_mdia(
    timescale: u32,
    duration: u64,
    sample_rate: u32,
    channels: u16,
    codec_private: &[u8],
) -> Vec<u8> {
    let mdhd = write_mdhd(timescale, duration);
    let hdlr = write_hdlr(b"soun", b"SoundHandler");
    let minf = write_audio_minf(sample_rate, channels, codec_private);
    write_container_box(b"mdia", &[&mdhd, &hdlr, &minf])
}

// ---------------------------------------------------------------------------
// trak (track) container
// ---------------------------------------------------------------------------

pub(crate) fn write_video_trak(
    track_id: u32,
    timescale: u32,
    duration: u64,
    codec: &Codec,
    width: u32,
    height: u32,
    codec_private: &[u8],
) -> Vec<u8> {
    let tkhd = write_tkhd(track_id, duration, true, width, height);
    let mdia = write_video_mdia(timescale, duration, codec, width, height, codec_private);
    write_container_box(b"trak", &[&tkhd, &mdia])
}

pub(crate) fn write_audio_trak(
    track_id: u32,
    timescale: u32,
    duration: u64,
    sample_rate: u32,
    channels: u16,
    codec_private: &[u8],
) -> Vec<u8> {
    let tkhd = write_tkhd(track_id, duration, false, 0, 0);
    let mdia = write_audio_mdia(timescale, duration, sample_rate, channels, codec_private);
    write_container_box(b"trak", &[&tkhd, &mdia])
}

// ---------------------------------------------------------------------------
// trex box (track extends, for mvex)
// ---------------------------------------------------------------------------

pub(crate) fn write_trex(track_id: u32) -> Vec<u8> {
    let mut content = Vec::with_capacity(24);
    content.extend_from_slice(&fullbox_header(0, 0));
    content.extend_from_slice(&track_id.to_be_bytes());
    content.extend_from_slice(&1u32.to_be_bytes()); // default sample description index
    content.extend_from_slice(&0u32.to_be_bytes()); // default sample duration
    content.extend_from_slice(&0u32.to_be_bytes()); // default sample size
    content.extend_from_slice(&0u32.to_be_bytes()); // default sample flags
    write_box(b"trex", &content)
}

// ---------------------------------------------------------------------------
// mvex (movie extends) container
// ---------------------------------------------------------------------------

pub(crate) fn write_mvex(track_id: u32) -> Vec<u8> {
    let trex = write_trex(track_id);
    write_container_box(b"mvex", &[&trex])
}

// ---------------------------------------------------------------------------
// moov (movie) container
// ---------------------------------------------------------------------------

pub(crate) fn write_moov(
    timescale: u32,
    duration: u64,
    trak: &[u8],
    mvex: &[u8],
) -> Vec<u8> {
    let mvhd = write_mvhd(timescale, duration);
    write_container_box(b"moov", &[&mvhd, trak, mvex])
}

// ---------------------------------------------------------------------------
// moof boxes (movie fragment)
// ---------------------------------------------------------------------------

/// Write the `mfhd` box (movie fragment header).
pub(crate) fn write_mfhd(sequence_number: u32) -> Vec<u8> {
    let mut content = Vec::with_capacity(8);
    content.extend_from_slice(&fullbox_header(0, 0));
    content.extend_from_slice(&sequence_number.to_be_bytes());
    write_box(b"mfhd", &content)
}

/// Write the `tfhd` box (track fragment header).
/// Uses default-base-is-moof flag (0x020000).
pub(crate) fn write_tfhd(track_id: u32) -> Vec<u8> {
    let mut content = Vec::with_capacity(8);
    // version 0, flags = 0x020000 (default-base-is-moof)
    content.extend_from_slice(&fullbox_header(0, 0x020000));
    content.extend_from_slice(&track_id.to_be_bytes());
    write_box(b"tfhd", &content)
}

/// Write the `tfdt` box (track fragment decode time, version 1 for 64-bit).
pub(crate) fn write_tfdt(base_media_decode_time: u64) -> Vec<u8> {
    let mut content = Vec::with_capacity(12);
    content.extend_from_slice(&fullbox_header(1, 0));
    content.extend_from_slice(&base_media_decode_time.to_be_bytes());
    write_box(b"tfdt", &content)
}

/// A single sample's info for `trun`.
pub(crate) struct TrunSample {
    pub size: u32,
    pub flags: u32,
    pub composition_time_offset: i32,
}

/// Write the `trun` box (track run).
///
/// Flags used:
/// - 0x000001: data-offset-present
/// - 0x000200: sample-size-present
/// - 0x000400: sample-flags-present
/// - 0x000800: sample-composition-time-offset-present
///
/// `data_offset` is the offset from the start of the containing moof to the
/// first byte of sample data in mdat.
pub(crate) fn write_trun(samples: &[TrunSample], data_offset: i32) -> Vec<u8> {
    let trun_flags: u32 = 0x000001 | 0x000200 | 0x000400 | 0x000800;
    let mut content = Vec::with_capacity(8 + 4 + samples.len() * 12);
    // version 1 (signed composition offsets), flags
    content.extend_from_slice(&fullbox_header(1, trun_flags));
    // sample count
    content.extend_from_slice(&(samples.len() as u32).to_be_bytes());
    // data offset
    content.extend_from_slice(&data_offset.to_be_bytes());
    // per-sample fields
    for s in samples {
        content.extend_from_slice(&s.size.to_be_bytes());
        content.extend_from_slice(&s.flags.to_be_bytes());
        content.extend_from_slice(&s.composition_time_offset.to_be_bytes());
    }
    write_box(b"trun", &content)
}

/// Write the `mdat` box header (just the header, caller appends data).
/// Returns (header_bytes, header_size).
pub(crate) fn write_mdat_header(data_size: u64) -> Vec<u8> {
    if data_size + 8 > u32::MAX as u64 {
        // Extended size
        let mut hdr = Vec::with_capacity(16);
        hdr.extend_from_slice(&1u32.to_be_bytes());
        hdr.extend_from_slice(b"mdat");
        hdr.extend_from_slice(&(data_size + 16).to_be_bytes());
        hdr
    } else {
        let mut hdr = Vec::with_capacity(8);
        hdr.extend_from_slice(&((data_size + 8) as u32).to_be_bytes());
        hdr.extend_from_slice(b"mdat");
        hdr
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: read a big-endian u32 from a slice.
    fn read_u32(data: &[u8], offset: usize) -> u32 {
        u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ])
    }

    #[test]
    fn test_write_box_size_and_type() {
        let b = write_box(b"test", &[1, 2, 3]);
        assert_eq!(b.len(), 11); // 8 header + 3 content
        assert_eq!(read_u32(&b, 0), 11);
        assert_eq!(&b[4..8], b"test");
        assert_eq!(&b[8..], &[1, 2, 3]);
    }

    #[test]
    fn test_write_container_box() {
        let child1 = write_box(b"ch_1", &[0xAA]);
        let child2 = write_box(b"ch_2", &[0xBB, 0xCC]);
        let container = write_container_box(b"cont", &[&child1, &child2]);
        // 8 (container header) + 9 (child1) + 10 (child2) = 27
        assert_eq!(container.len(), 27);
        assert_eq!(read_u32(&container, 0), 27);
        assert_eq!(&container[4..8], b"cont");
    }

    #[test]
    fn test_ftyp_box() {
        let ftyp = write_ftyp();
        // 8 header + 4 major + 4 minor + 3*4 compatible = 28
        assert_eq!(ftyp.len(), 28);
        assert_eq!(read_u32(&ftyp, 0), 28);
        assert_eq!(&ftyp[4..8], b"ftyp");
        assert_eq!(&ftyp[8..12], b"isom"); // major brand
    }

    #[test]
    fn test_mvhd_box_size() {
        let mvhd = write_mvhd(90000, 0);
        // version-1 mvhd is 120 bytes total (8 header + 112 content)
        assert_eq!(mvhd.len(), 120);
        assert_eq!(read_u32(&mvhd, 0), 120);
        assert_eq!(&mvhd[4..8], b"mvhd");
    }

    #[test]
    fn test_tkhd_box_size() {
        let tkhd = write_tkhd(1, 1000, true, 1920, 1080);
        // version-1 tkhd is 104 bytes (8 header + 96 content)
        assert_eq!(tkhd.len(), 104);
        assert_eq!(read_u32(&tkhd, 0), 104);
    }

    #[test]
    fn test_mdhd_box_size() {
        let mdhd = write_mdhd(90000, 0);
        // version-1 mdhd is 44 bytes (8 header + 36 content)
        assert_eq!(mdhd.len(), 44);
        assert_eq!(read_u32(&mdhd, 0), 44);
    }

    #[test]
    fn test_mdat_header_normal() {
        let hdr = write_mdat_header(100);
        assert_eq!(hdr.len(), 8);
        assert_eq!(read_u32(&hdr, 0), 108); // 100 + 8
        assert_eq!(&hdr[4..8], b"mdat");
    }

    #[test]
    fn test_mdat_header_extended() {
        let hdr = write_mdat_header(u32::MAX as u64);
        assert_eq!(hdr.len(), 16);
        assert_eq!(read_u32(&hdr, 0), 1); // extended size marker
        assert_eq!(&hdr[4..8], b"mdat");
    }
}
