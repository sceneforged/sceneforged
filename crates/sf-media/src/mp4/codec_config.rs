//! Extract codec configuration (avcC and esds) from MP4 sample description boxes.

use std::io::{self, Read, Seek, SeekFrom};

use super::atoms::{find_child_box, read_bytes, read_fullbox_header, read_u32};

/// Extract the raw avcC box content from an stsd containing an avc1 sample entry.
///
/// Returns the raw bytes of the avcC configuration (SPS/PPS data).
/// The reader should be positioned at the start of stsd content.
pub fn extract_avcc<R: Read + Seek>(reader: &mut R, stsd_content_size: u64) -> io::Result<Option<Vec<u8>>> {
    let start = reader.stream_position()?;

    // Read past fullbox header (4 bytes) and entry_count (4 bytes).
    read_fullbox_header(reader)?;
    let entry_count = read_u32(reader)?;
    if entry_count == 0 {
        return Ok(None);
    }

    let remaining = stsd_content_size - (reader.stream_position()? - start);

    // Find the avc1 (or avc3) sample entry box.
    let avc1_header = if let Some(h) = find_child_box(reader, remaining, b"avc1")? {
        h
    } else {
        reader.seek(SeekFrom::Start(start + 8))?;
        let remaining = stsd_content_size - 8;
        if let Some(h) = find_child_box(reader, remaining, b"avc3")? {
            h
        } else {
            return Ok(None);
        }
    };

    // Inside avc1: skip visual sample entry fields (78 bytes from content start)
    // to get to child boxes.
    let entry_start = reader.stream_position()?;
    let entry_content = avc1_header.content_size();
    if entry_content < 78 {
        return Ok(None);
    }
    reader.seek(SeekFrom::Current(78))?;
    let child_content = entry_content - 78;

    // Find avcC child box.
    if let Some(avcc_header) = find_child_box(reader, child_content, b"avcC")? {
        let data = read_bytes(reader, avcc_header.content_size() as usize)?;
        return Ok(Some(data));
    }

    // Seek past this entry in case we missed something.
    reader.seek(SeekFrom::Start(entry_start + entry_content))?;
    Ok(None)
}

/// Extract the raw esds box content from an stsd containing an mp4a sample entry.
///
/// Returns the full esds fullbox content (version+flags + ES_Descriptor).
/// The reader should be positioned at the start of stsd content.
pub fn extract_esds<R: Read + Seek>(reader: &mut R, stsd_content_size: u64) -> io::Result<Option<Vec<u8>>> {
    let start = reader.stream_position()?;

    // Read past fullbox header (4 bytes) and entry_count (4 bytes).
    read_fullbox_header(reader)?;
    let entry_count = read_u32(reader)?;
    if entry_count == 0 {
        return Ok(None);
    }

    let remaining = stsd_content_size - (reader.stream_position()? - start);

    // Find the mp4a sample entry box.
    let mp4a_header = match find_child_box(reader, remaining, b"mp4a")? {
        Some(h) => h,
        None => return Ok(None),
    };

    // Inside mp4a: skip audio sample entry fields (28 bytes from content start).
    let entry_start = reader.stream_position()?;
    let entry_content = mp4a_header.content_size();
    if entry_content < 28 {
        return Ok(None);
    }
    reader.seek(SeekFrom::Current(28))?;
    let child_content = entry_content - 28;

    // Find esds child box.
    if let Some(esds_header) = find_child_box(reader, child_content, b"esds")? {
        let data = read_bytes(reader, esds_header.content_size() as usize)?;
        return Ok(Some(data));
    }

    reader.seek(SeekFrom::Start(entry_start + entry_content))?;
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fmp4::boxes;
    use std::io::Cursor;

    #[test]
    fn test_extract_avcc_from_stsd() {
        // Build a minimal stsd > avc1 > avcC structure.
        let avcc_data = vec![
            0x01, // configurationVersion
            0x64, // AVCProfileIndication (High)
            0x00, // profile_compatibility
            0x1F, // AVCLevelIndication (3.1)
            0xFC | 3, // lengthSizeMinusOne=3 (4-byte NALUs)
            0xE0 | 1, // numOfSequenceParameterSets=1
            0x00, 0x04, // SPS length
            0x67, 0x64, 0x00, 0x1F, // SPS data
            0x01, // numOfPictureParameterSets
            0x00, 0x02, // PPS length
            0x68, 0xEE, // PPS data
        ];
        let avcc_box = boxes::write_box(b"avcC", &avcc_data);

        // Visual sample entry header (78 bytes).
        let mut avc1_content = Vec::new();
        avc1_content.extend_from_slice(&[0u8; 6]); // reserved
        avc1_content.extend_from_slice(&1u16.to_be_bytes()); // data_ref_index
        avc1_content.extend_from_slice(&[0u8; 16]); // pre_defined + reserved
        avc1_content.extend_from_slice(&1920u16.to_be_bytes()); // width
        avc1_content.extend_from_slice(&1080u16.to_be_bytes()); // height
        avc1_content.extend_from_slice(&0x00480000u32.to_be_bytes()); // horiz res
        avc1_content.extend_from_slice(&0x00480000u32.to_be_bytes()); // vert res
        avc1_content.extend_from_slice(&0u32.to_be_bytes()); // reserved
        avc1_content.extend_from_slice(&1u16.to_be_bytes()); // frame count
        avc1_content.extend_from_slice(&[0u8; 32]); // compressor name
        avc1_content.extend_from_slice(&0x0018u16.to_be_bytes()); // depth
        avc1_content.extend_from_slice(&(-1i16).to_be_bytes()); // pre_defined
        avc1_content.extend_from_slice(&avcc_box);

        let avc1_box = boxes::write_box(b"avc1", &avc1_content);

        // stsd fullbox
        let mut stsd_content = Vec::new();
        stsd_content.extend_from_slice(&boxes::fullbox_header(0, 0));
        stsd_content.extend_from_slice(&1u32.to_be_bytes()); // entry count
        stsd_content.extend_from_slice(&avc1_box);

        let mut cursor = Cursor::new(&stsd_content);
        let result = extract_avcc(&mut cursor, stsd_content.len() as u64)
            .unwrap()
            .unwrap();
        assert_eq!(result, avcc_data);
    }

    #[test]
    fn test_extract_esds_from_stsd() {
        let esds_data = vec![
            0x00, 0x00, 0x00, 0x00, // version + flags
            0x03, 0x19, // ES_Descriptor tag + length
            0x00, 0x01, // ES_ID
            0x00, // streamDependenceFlag, etc.
            0x04, 0x11, // DecoderConfigDescriptor tag + length
            0x40, // objectTypeIndication (AAC)
            0x15, // streamType (audio)
            0x00, 0x00, 0x00, // bufferSizeDB
            0x00, 0x01, 0xF4, 0x00, // maxBitrate
            0x00, 0x01, 0xF4, 0x00, // avgBitrate
            0x05, 0x02, // AudioSpecificConfig tag + length
            0x12, 0x10, // AAC-LC, 44100Hz, stereo
            0x06, 0x01, 0x02, // SLConfigDescriptor
        ];
        let esds_box = boxes::write_box(b"esds", &esds_data);

        // Audio sample entry (28 bytes header).
        let mut mp4a_content = Vec::new();
        mp4a_content.extend_from_slice(&[0u8; 6]); // reserved
        mp4a_content.extend_from_slice(&1u16.to_be_bytes()); // data_ref_index
        mp4a_content.extend_from_slice(&[0u8; 8]); // reserved
        mp4a_content.extend_from_slice(&2u16.to_be_bytes()); // channels
        mp4a_content.extend_from_slice(&16u16.to_be_bytes()); // sample_size
        mp4a_content.extend_from_slice(&0u16.to_be_bytes()); // pre_defined
        mp4a_content.extend_from_slice(&0u16.to_be_bytes()); // reserved
        mp4a_content.extend_from_slice(&(48000u32 << 16).to_be_bytes()); // sample_rate
        mp4a_content.extend_from_slice(&esds_box);

        let mp4a_box = boxes::write_box(b"mp4a", &mp4a_content);

        let mut stsd_content = Vec::new();
        stsd_content.extend_from_slice(&boxes::fullbox_header(0, 0));
        stsd_content.extend_from_slice(&1u32.to_be_bytes());
        stsd_content.extend_from_slice(&mp4a_box);

        let mut cursor = Cursor::new(&stsd_content);
        let result = extract_esds(&mut cursor, stsd_content.len() as u64)
            .unwrap()
            .unwrap();
        assert_eq!(result, esds_data);
    }
}
