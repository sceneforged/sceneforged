//! MP4 moov atom parser.
//!
//! Parses an existing MP4 file's moov atom to extract sample tables and codec
//! configuration for zero-copy HLS serving.

pub mod atoms;
pub mod codec_config;
pub mod sample_table;

use std::io::{self, Read, Seek, SeekFrom};

use atoms::{find_child_box, read_box_header, read_fullbox_header, read_u16, read_u32, read_u64, skip_box};
pub use sample_table::{ResolvedSample, ResolvedSampleTable};

/// Parsed metadata from an MP4 file's moov atom.
#[derive(Debug, Clone)]
pub struct Mp4Metadata {
    pub video_track: Option<TrackInfo>,
    pub audio_track: Option<TrackInfo>,
    pub duration_secs: f64,
}

/// Information about a single track.
#[derive(Debug, Clone)]
pub struct TrackInfo {
    pub track_id: u32,
    pub handler_type: [u8; 4],
    pub timescale: u32,
    pub duration: u64,
    pub width: u32,
    pub height: u32,
    pub sample_rate: u32,
    pub channels: u16,
    pub codec_private: Vec<u8>,
    pub sample_table: ResolvedSampleTable,
}

/// Parse the moov atom from an MP4 file.
///
/// The reader should be positioned at the beginning of the file (or at least
/// before the moov atom).
pub fn parse_moov<R: Read + Seek>(reader: &mut R) -> io::Result<Mp4Metadata> {
    // Scan top-level boxes for moov.
    reader.seek(SeekFrom::Start(0))?;

    let file_size = {
        let pos = reader.stream_position()?;
        let end = reader.seek(SeekFrom::End(0))?;
        reader.seek(SeekFrom::Start(pos))?;
        end
    };

    let mut moov_header = None;
    while reader.stream_position()? < file_size {
        let Some(header) = read_box_header(reader)? else {
            break;
        };
        if header.size == 0 {
            break;
        }
        if &header.box_type == b"moov" {
            moov_header = Some(header);
            break;
        }
        skip_box(reader, &header)?;
    }

    let moov = moov_header
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "No moov atom found"))?;
    let moov_content_size = moov.content_size();
    let moov_start = reader.stream_position()?;

    // Parse mvhd to get global timescale and duration.
    let (global_timescale, global_duration) = {
        reader.seek(SeekFrom::Start(moov_start))?;
        let mvhd = find_child_box(reader, moov_content_size, b"mvhd")?
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "No mvhd in moov"))?;
        parse_mvhd(reader, &mvhd)?
    };

    let duration_secs = if global_timescale > 0 {
        global_duration as f64 / global_timescale as f64
    } else {
        0.0
    };

    // Iterate trak boxes.
    let mut video_track = None;
    let mut audio_track = None;

    let mut pos = moov_start;
    let moov_end = moov_start + moov_content_size;

    while pos < moov_end {
        reader.seek(SeekFrom::Start(pos))?;
        let Some(header) = read_box_header(reader)? else {
            break;
        };
        if header.size == 0 {
            break;
        }

        if &header.box_type == b"trak" {
            let trak_start = reader.stream_position()?;
            let trak_content = header.content_size();

            if let Ok(Some(track)) = parse_trak(reader, trak_start, trak_content) {
                match &track.handler_type {
                    b"vide" if video_track.is_none() => video_track = Some(track),
                    b"soun" if audio_track.is_none() => audio_track = Some(track),
                    _ => {}
                }
            }
        }

        pos += header.size;
    }

    Ok(Mp4Metadata {
        video_track,
        audio_track,
        duration_secs,
    })
}

/// Parse mvhd, return (timescale, duration).
fn parse_mvhd<R: Read + Seek>(reader: &mut R, _header: &atoms::BoxHeader) -> io::Result<(u32, u64)> {
    let (version, _flags) = read_fullbox_header(reader)?;
    if version == 1 {
        let _creation_time = read_u64(reader)?;
        let _modification_time = read_u64(reader)?;
        let timescale = read_u32(reader)?;
        let duration = read_u64(reader)?;
        Ok((timescale, duration))
    } else {
        let _creation_time = read_u32(reader)?;
        let _modification_time = read_u32(reader)?;
        let timescale = read_u32(reader)?;
        let duration = read_u32(reader)? as u64;
        Ok((timescale, duration))
    }
}

/// Parse a single trak box, returning TrackInfo if it's a video or audio track.
fn parse_trak<R: Read + Seek>(
    reader: &mut R,
    trak_start: u64,
    trak_content: u64,
) -> io::Result<Option<TrackInfo>> {
    // Parse tkhd.
    reader.seek(SeekFrom::Start(trak_start))?;
    let tkhd = match find_child_box(reader, trak_content, b"tkhd")? {
        Some(h) => h,
        None => return Ok(None),
    };
    let (track_id, tkhd_width, tkhd_height) = parse_tkhd(reader, &tkhd)?;

    // Find mdia container.
    reader.seek(SeekFrom::Start(trak_start))?;
    let mdia = match find_child_box(reader, trak_content, b"mdia")? {
        Some(h) => h,
        None => return Ok(None),
    };
    let mdia_start = reader.stream_position()?;
    let mdia_content = mdia.content_size();

    // Parse mdhd for timescale/duration.
    reader.seek(SeekFrom::Start(mdia_start))?;
    let mdhd = match find_child_box(reader, mdia_content, b"mdhd")? {
        Some(h) => h,
        None => return Ok(None),
    };
    let (timescale, duration) = parse_mdhd(reader, &mdhd)?;

    // Parse hdlr for handler_type.
    reader.seek(SeekFrom::Start(mdia_start))?;
    let hdlr = match find_child_box(reader, mdia_content, b"hdlr")? {
        Some(h) => h,
        None => return Ok(None),
    };
    let handler_type = parse_hdlr(reader, &hdlr)?;

    // Only process video/audio tracks.
    if &handler_type != b"vide" && &handler_type != b"soun" {
        return Ok(None);
    }

    // Find minf > stbl.
    reader.seek(SeekFrom::Start(mdia_start))?;
    let minf = match find_child_box(reader, mdia_content, b"minf")? {
        Some(h) => h,
        None => return Ok(None),
    };
    let minf_start = reader.stream_position()?;
    let minf_content = minf.content_size();

    reader.seek(SeekFrom::Start(minf_start))?;
    let stbl = match find_child_box(reader, minf_content, b"stbl")? {
        Some(h) => h,
        None => return Ok(None),
    };
    let stbl_start = reader.stream_position()?;
    let stbl_content = stbl.content_size();

    // Extract codec config from stsd inside stbl.
    let (codec_private, sample_rate, channels) = if &handler_type == b"vide" {
        // Look for avcC.
        reader.seek(SeekFrom::Start(stbl_start))?;
        let stsd = find_child_box(reader, stbl_content, b"stsd")?;
        let cp = if let Some(stsd_h) = stsd {
            let stsd_pos = reader.stream_position()?;
            let stsd_content_size = stsd_h.content_size();
            reader.seek(SeekFrom::Start(stsd_pos))?;
            codec_config::extract_avcc(reader, stsd_content_size)?
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        (cp, 0, 0)
    } else {
        // Look for esds.
        reader.seek(SeekFrom::Start(stbl_start))?;
        let stsd = find_child_box(reader, stbl_content, b"stsd")?;
        let (cp, sr, ch) = if let Some(stsd_h) = stsd {
            let stsd_pos = reader.stream_position()?;
            let stsd_content_size = stsd_h.content_size();
            // Extract sample rate and channels from mp4a header.
            let (sr, ch) = extract_audio_params(reader, stsd_content_size)?;
            reader.seek(SeekFrom::Start(stsd_pos))?;
            let esds = codec_config::extract_esds(reader, stsd_content_size)?
                .unwrap_or_default();
            (esds, sr, ch)
        } else {
            (Vec::new(), 0, 0)
        };
        (cp, sr, ch)
    };

    // Parse sample table.
    reader.seek(SeekFrom::Start(stbl_start))?;
    let sample_table = sample_table::resolve_sample_table(reader, stbl_content, timescale)?;

    Ok(Some(TrackInfo {
        track_id,
        handler_type,
        timescale,
        duration,
        width: tkhd_width,
        height: tkhd_height,
        sample_rate,
        channels,
        codec_private,
        sample_table,
    }))
}

/// Parse tkhd, return (track_id, width, height). Width/height are 16.16 fixed point.
fn parse_tkhd<R: Read + Seek>(reader: &mut R, _header: &atoms::BoxHeader) -> io::Result<(u32, u32, u32)> {
    let (version, _flags) = read_fullbox_header(reader)?;
    if version == 1 {
        let _creation = read_u64(reader)?;
        let _modification = read_u64(reader)?;
        let track_id = read_u32(reader)?;
        let _reserved = read_u32(reader)?;
        let _duration = read_u64(reader)?;
        // Skip: reserved(8) + layer(2) + alt_group(2) + volume(2) + reserved(2) + matrix(36)
        let mut skip = [0u8; 52];
        reader.read_exact(&mut skip)?;
        let width = read_u32(reader)? >> 16;
        let height = read_u32(reader)? >> 16;
        Ok((track_id, width, height))
    } else {
        let _creation = read_u32(reader)?;
        let _modification = read_u32(reader)?;
        let track_id = read_u32(reader)?;
        let _reserved = read_u32(reader)?;
        let _duration = read_u32(reader)?;
        let mut skip = [0u8; 52];
        reader.read_exact(&mut skip)?;
        let width = read_u32(reader)? >> 16;
        let height = read_u32(reader)? >> 16;
        Ok((track_id, width, height))
    }
}

/// Parse mdhd, return (timescale, duration).
fn parse_mdhd<R: Read>(reader: &mut R, _header: &atoms::BoxHeader) -> io::Result<(u32, u64)> {
    let (version, _flags) = read_fullbox_header(reader)?;
    if version == 1 {
        let _creation = read_u64(reader)?;
        let _modification = read_u64(reader)?;
        let timescale = read_u32(reader)?;
        let duration = read_u64(reader)?;
        Ok((timescale, duration))
    } else {
        let _creation = read_u32(reader)?;
        let _modification = read_u32(reader)?;
        let timescale = read_u32(reader)?;
        let duration = read_u32(reader)? as u64;
        Ok((timescale, duration))
    }
}

/// Parse hdlr, return handler_type.
fn parse_hdlr<R: Read>(reader: &mut R, _header: &atoms::BoxHeader) -> io::Result<[u8; 4]> {
    let (_version, _flags) = read_fullbox_header(reader)?;
    let _pre_defined = read_u32(reader)?;
    let mut handler = [0u8; 4];
    reader.read_exact(&mut handler)?;
    Ok(handler)
}

/// Extract audio sample rate and channel count from mp4a sample entry inside stsd.
fn extract_audio_params<R: Read + Seek>(reader: &mut R, stsd_content_size: u64) -> io::Result<(u32, u16)> {
    let start = reader.stream_position()?;
    // Skip fullbox header (4) + entry_count (4).
    read_fullbox_header(reader)?;
    let _entry_count = read_u32(reader)?;

    let remaining = stsd_content_size - (reader.stream_position()? - start);

    let mp4a = match find_child_box(reader, remaining, b"mp4a")? {
        Some(h) => h,
        None => return Ok((0, 0)),
    };
    let _ = mp4a;

    // Inside mp4a sample entry:
    // reserved (6) + data_ref_index (2) + reserved (8) = 16 bytes
    // then channel_count (2) + sample_size (2) + pre_defined (2) + reserved (2) = 8 bytes
    // then sample_rate (4, fixed 16.16)
    let mut skip = [0u8; 16];
    reader.read_exact(&mut skip)?;
    let channels = read_u16(reader)?;
    let _sample_size = read_u16(reader)?;
    let _pre_defined = read_u16(reader)?;
    let _reserved = read_u16(reader)?;
    let sr_fixed = read_u32(reader)?;
    let sample_rate = sr_fixed >> 16;

    Ok((sample_rate, channels))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fmp4;

    /// Build a minimal MP4 with the fmp4 writer, then parse it back.
    #[test]
    fn test_parse_moov_roundtrip() {
        // Create a video init segment using the existing writer.
        let config = fmp4::TrackConfig {
            track_id: 1,
            timescale: 90000,
            codec: fmp4::Codec::Avc,
            width: 1920,
            height: 1080,
            sample_rate: 0,
            channels: 0,
            codec_private: vec![
                0x01, 0x64, 0x00, 0x1F, // avcC header bytes
                0xFC | 3, // lengthSizeMinusOne
                0xE0 | 1, // numSPS
                0x00, 0x04, // SPS length
                0x67, 0x64, 0x00, 0x1F, // SPS
                0x01, // numPPS
                0x00, 0x02, // PPS length
                0x68, 0xEE, // PPS
            ],
        };

        let init = fmp4::write_init_segment(&config);

        // Add a segment with sample data so we have something to parse.
        let samples = vec![
            fmp4::SampleInfo {
                data: vec![0xAA; 100],
                duration: 3000,
                is_keyframe: true,
                composition_offset: 0,
            },
            fmp4::SampleInfo {
                data: vec![0xBB; 50],
                duration: 3000,
                is_keyframe: false,
                composition_offset: 1500,
            },
        ];
        let segment = fmp4::write_media_segment(1, 0, &samples);

        // Concatenate init + segment into a complete MP4 file.
        let mut mp4_data = init;
        mp4_data.extend_from_slice(&segment);

        // Parse it.
        let mut cursor = std::io::Cursor::new(&mp4_data);
        let metadata = parse_moov(&mut cursor).unwrap();

        // We should find a video track.
        assert!(metadata.video_track.is_some());
        assert!(metadata.audio_track.is_none());

        let video = metadata.video_track.unwrap();
        assert_eq!(video.track_id, 1);
        assert_eq!(video.timescale, 90000);
        assert_eq!(video.width, 1920);
        assert_eq!(video.height, 1080);
        assert_eq!(&video.handler_type, b"vide");

        // The init segment has empty sample tables (fragmented MP4),
        // so we expect 0 samples in the parsed moov.
        assert_eq!(video.sample_table.samples.len(), 0);
    }
}
