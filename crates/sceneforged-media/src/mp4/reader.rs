//! MP4 file reader with atom parsing.

use super::{Atom, AtomType, HandlerType, Mp4File, SampleTableBuilder, TrackInfo};
use crate::Result;
use std::io::{Read, Seek, SeekFrom};

/// Maximum allowed atom data size (64 MB) to prevent OOM on malformed files.
const MAX_ATOM_DATA_SIZE: u64 = 64 * 1024 * 1024;

/// MP4 file reader.
pub struct Mp4Reader<R> {
    reader: R,
    file_size: u64,
}

impl<R: Read + Seek> Mp4Reader<R> {
    /// Create a new MP4 reader.
    pub fn new(mut reader: R) -> Self {
        let file_size = reader.seek(SeekFrom::End(0)).unwrap_or(0);
        let _ = reader.seek(SeekFrom::Start(0));
        Self { reader, file_size }
    }

    /// Parse the MP4 file.
    pub fn parse(&mut self) -> Result<Mp4File> {
        let mut mp4 = Mp4File {
            duration: 0,
            timescale: 1000,
            video_track: None,
            audio_track: None,
            has_faststart: false,
        };

        let mut moov_offset = 0u64;
        let mut mdat_offset = 0u64;

        // First pass: find top-level atoms
        let atoms = self.read_atoms(0, self.file_size)?;

        for atom in &atoms {
            match atom.atom_type {
                AtomType::MOOV => {
                    moov_offset = atom.data_offset - atom.header_size as u64;
                    self.parse_moov(atom, &mut mp4)?;
                }
                AtomType::MDAT => {
                    mdat_offset = atom.data_offset - atom.header_size as u64;
                }
                _ => {}
            }
        }

        // Faststart means moov comes before mdat
        mp4.has_faststart = moov_offset < mdat_offset || mdat_offset == 0;

        Ok(mp4)
    }

    /// Read atoms at the given level.
    fn read_atoms(&mut self, start: u64, end: u64) -> Result<Vec<Atom>> {
        let mut atoms = Vec::new();
        let mut pos = start;

        while pos < end {
            self.reader.seek(SeekFrom::Start(pos))?;

            // Read atom header
            let mut header = [0u8; 8];
            if self.reader.read_exact(&mut header).is_err() {
                break;
            }

            let size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
            let atom_type = AtomType::from_bytes([header[4], header[5], header[6], header[7]]);

            let (actual_size, header_size) = if size == 1 {
                // 64-bit extended size
                let mut ext = [0u8; 8];
                self.reader.read_exact(&mut ext)?;
                (u64::from_be_bytes(ext), 16u8)
            } else if size == 0 {
                // Atom extends to end of file
                (end - pos, 8u8)
            } else {
                (size, 8u8)
            };

            if actual_size < header_size as u64 {
                break;
            }

            atoms.push(Atom {
                atom_type,
                size: actual_size,
                data_offset: pos + header_size as u64,
                header_size,
            });

            pos += actual_size;
        }

        Ok(atoms)
    }

    /// Read and validate atom data, rejecting oversized atoms.
    fn read_atom_data(&mut self, atom: &Atom) -> Result<Vec<u8>> {
        let size = atom.data_size();
        if size > MAX_ATOM_DATA_SIZE {
            return Err(crate::Error::InvalidMp4(format!(
                "Atom {} data size {} exceeds maximum {}",
                atom.atom_type, size, MAX_ATOM_DATA_SIZE
            )));
        }
        self.reader.seek(SeekFrom::Start(atom.data_offset))?;
        let mut data = vec![0u8; size as usize];
        self.reader.read_exact(&mut data)?;
        Ok(data)
    }

    /// Parse moov atom.
    fn parse_moov(&mut self, moov: &Atom, mp4: &mut Mp4File) -> Result<()> {
        let children = self.read_atoms(moov.data_offset, moov.data_offset + moov.data_size())?;

        for child in &children {
            match child.atom_type {
                AtomType::MVHD => {
                    self.parse_mvhd(child, mp4)?;
                }
                AtomType::TRAK => {
                    if let Ok(track) = self.parse_trak(child) {
                        match track.handler_type {
                            HandlerType::Video if mp4.video_track.is_none() => {
                                mp4.video_track = Some(track);
                            }
                            HandlerType::Audio if mp4.audio_track.is_none() => {
                                mp4.audio_track = Some(track);
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Parse mvhd (movie header).
    fn parse_mvhd(&mut self, atom: &Atom, mp4: &mut Mp4File) -> Result<()> {
        let data = self.read_atom_data(atom)?;

        if data.is_empty() {
            return Ok(());
        }

        let version = data[0];

        if version == 0 {
            // 32-bit timestamps
            if data.len() >= 20 {
                mp4.timescale = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
                mp4.duration = u32::from_be_bytes([data[16], data[17], data[18], data[19]]) as u64;
            }
        } else {
            // 64-bit timestamps
            if data.len() >= 28 {
                mp4.timescale = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
                mp4.duration = u64::from_be_bytes([
                    data[24], data[25], data[26], data[27], data[28], data[29], data[30], data[31],
                ]);
            }
        }

        Ok(())
    }

    /// Parse trak (track) atom.
    fn parse_trak(&mut self, trak: &Atom) -> Result<TrackInfo> {
        let children = self.read_atoms(trak.data_offset, trak.data_offset + trak.data_size())?;

        let mut track = TrackInfo::new(0);

        for child in &children {
            match child.atom_type {
                AtomType::TKHD => {
                    self.parse_tkhd(child, &mut track)?;
                }
                AtomType::MDIA => {
                    self.parse_mdia(child, &mut track)?;
                }
                _ => {}
            }
        }

        Ok(track)
    }

    /// Parse tkhd (track header).
    fn parse_tkhd(&mut self, atom: &Atom, track: &mut TrackInfo) -> Result<()> {
        let data = self.read_atom_data(atom)?;

        if data.is_empty() {
            return Ok(());
        }

        let version = data[0];

        if version == 0 {
            if data.len() >= 12 {
                track.track_id = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
            }
            if data.len() >= 84 {
                // Width and height at fixed point 16.16
                let w = u32::from_be_bytes([data[76], data[77], data[78], data[79]]);
                let h = u32::from_be_bytes([data[80], data[81], data[82], data[83]]);
                track.width = Some(w >> 16);
                track.height = Some(h >> 16);
            }
        } else {
            if data.len() >= 20 {
                track.track_id = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
            }
            if data.len() >= 92 {
                let w = u32::from_be_bytes([data[84], data[85], data[86], data[87]]);
                let h = u32::from_be_bytes([data[88], data[89], data[90], data[91]]);
                track.width = Some(w >> 16);
                track.height = Some(h >> 16);
            }
        }

        Ok(())
    }

    /// Parse mdia (media) atom.
    fn parse_mdia(&mut self, mdia: &Atom, track: &mut TrackInfo) -> Result<()> {
        let children = self.read_atoms(mdia.data_offset, mdia.data_offset + mdia.data_size())?;

        for child in &children {
            match child.atom_type {
                AtomType::MDHD => {
                    self.parse_mdhd(child, track)?;
                }
                AtomType::HDLR => {
                    self.parse_hdlr(child, track)?;
                }
                AtomType::MINF => {
                    self.parse_minf(child, track)?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Parse mdhd (media header).
    fn parse_mdhd(&mut self, atom: &Atom, track: &mut TrackInfo) -> Result<()> {
        let data = self.read_atom_data(atom)?;

        if data.is_empty() {
            return Ok(());
        }

        let version = data[0];

        if version == 0 {
            if data.len() >= 20 {
                track.timescale = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
                track.duration =
                    u32::from_be_bytes([data[16], data[17], data[18], data[19]]) as u64;
            }
        } else {
            if data.len() >= 28 {
                track.timescale = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
            }
            if data.len() >= 32 {
                track.duration = u64::from_be_bytes([
                    data[24], data[25], data[26], data[27], data[28], data[29], data[30], data[31],
                ]);
            }
        }

        Ok(())
    }

    /// Parse hdlr (handler) atom.
    fn parse_hdlr(&mut self, atom: &Atom, track: &mut TrackInfo) -> Result<()> {
        let data = self.read_atom_data(atom)?;

        if data.len() >= 12 {
            track.handler_type = HandlerType::from_bytes([data[8], data[9], data[10], data[11]]);
        }

        Ok(())
    }

    /// Parse minf (media info) atom.
    fn parse_minf(&mut self, minf: &Atom, track: &mut TrackInfo) -> Result<()> {
        let children = self.read_atoms(minf.data_offset, minf.data_offset + minf.data_size())?;

        for child in &children {
            if child.atom_type == AtomType::STBL {
                self.parse_stbl(child, track)?;
            }
        }

        Ok(())
    }

    /// Parse stbl (sample table) atom.
    fn parse_stbl(&mut self, stbl: &Atom, track: &mut TrackInfo) -> Result<()> {
        let children = self.read_atoms(stbl.data_offset, stbl.data_offset + stbl.data_size())?;

        let mut builder = SampleTableBuilder::new();

        for child in &children {
            match child.atom_type {
                AtomType::STTS => {
                    self.parse_stts(child, &mut builder)?;
                }
                AtomType::STSS => {
                    self.parse_stss(child, &mut builder)?;
                }
                AtomType::STSC => {
                    self.parse_stsc(child, &mut builder)?;
                }
                AtomType::STSZ => {
                    self.parse_stsz(child, &mut builder)?;
                }
                AtomType::STCO => {
                    self.parse_stco(child, &mut builder)?;
                }
                AtomType::CO64 => {
                    self.parse_co64(child, &mut builder)?;
                }
                AtomType::CTTS => {
                    self.parse_ctts(child, &mut builder)?;
                }
                AtomType::STSD => {
                    self.parse_stsd(child, track)?;
                }
                _ => {}
            }
        }

        track.sample_table = builder.build();
        Ok(())
    }

    /// Parse stts (decoding time to sample).
    fn parse_stts(&mut self, atom: &Atom, builder: &mut SampleTableBuilder) -> Result<()> {
        let data = self.read_atom_data(atom)?;

        if data.len() < 8 {
            return Ok(());
        }

        let entry_count = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let mut entries = Vec::with_capacity(entry_count);

        for i in 0..entry_count {
            let offset = 8 + i * 8;
            if offset + 8 > data.len() {
                break;
            }
            let count = u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let delta = u32::from_be_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            entries.push((count, delta));
        }

        builder.set_stts(entries);
        Ok(())
    }

    /// Parse stss (sync sample).
    fn parse_stss(&mut self, atom: &Atom, builder: &mut SampleTableBuilder) -> Result<()> {
        let data = self.read_atom_data(atom)?;

        if data.len() < 8 {
            return Ok(());
        }

        let entry_count = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let mut sync_samples = Vec::with_capacity(entry_count);

        for i in 0..entry_count {
            let offset = 8 + i * 4;
            if offset + 4 > data.len() {
                break;
            }
            let sample = u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            sync_samples.push(sample);
        }

        builder.set_sync_samples(sync_samples);
        Ok(())
    }

    /// Parse stsc (sample to chunk).
    fn parse_stsc(&mut self, atom: &Atom, builder: &mut SampleTableBuilder) -> Result<()> {
        let data = self.read_atom_data(atom)?;

        if data.len() < 8 {
            return Ok(());
        }

        let entry_count = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let mut entries = Vec::with_capacity(entry_count);

        for i in 0..entry_count {
            let offset = 8 + i * 12;
            if offset + 12 > data.len() {
                break;
            }
            let first_chunk = u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let samples_per_chunk = u32::from_be_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let description_idx = u32::from_be_bytes([
                data[offset + 8],
                data[offset + 9],
                data[offset + 10],
                data[offset + 11],
            ]);
            entries.push((first_chunk, samples_per_chunk, description_idx));
        }

        builder.set_stsc(entries);
        Ok(())
    }

    /// Parse stsz (sample size).
    fn parse_stsz(&mut self, atom: &Atom, builder: &mut SampleTableBuilder) -> Result<()> {
        let data = self.read_atom_data(atom)?;

        if data.len() < 12 {
            return Ok(());
        }

        let uniform_size = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let sample_count = u32::from_be_bytes([data[8], data[9], data[10], data[11]]) as usize;

        let sizes = if uniform_size == 0 {
            let mut sizes = Vec::with_capacity(sample_count);
            for i in 0..sample_count {
                let offset = 12 + i * 4;
                if offset + 4 > data.len() {
                    break;
                }
                let size = u32::from_be_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                sizes.push(size);
            }
            sizes
        } else {
            vec![]
        };

        builder.set_stsz(uniform_size, sizes);
        Ok(())
    }

    /// Parse stco (chunk offset, 32-bit).
    fn parse_stco(&mut self, atom: &Atom, builder: &mut SampleTableBuilder) -> Result<()> {
        let data = self.read_atom_data(atom)?;

        if data.len() < 8 {
            return Ok(());
        }

        let entry_count = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let mut offsets = Vec::with_capacity(entry_count);

        for i in 0..entry_count {
            let offset = 8 + i * 4;
            if offset + 4 > data.len() {
                break;
            }
            let chunk_offset = u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as u64;
            offsets.push(chunk_offset);
        }

        builder.set_chunk_offsets(offsets);
        Ok(())
    }

    /// Parse co64 (chunk offset, 64-bit).
    fn parse_co64(&mut self, atom: &Atom, builder: &mut SampleTableBuilder) -> Result<()> {
        let data = self.read_atom_data(atom)?;

        if data.len() < 8 {
            return Ok(());
        }

        let entry_count = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let mut offsets = Vec::with_capacity(entry_count);

        for i in 0..entry_count {
            let offset = 8 + i * 8;
            if offset + 8 > data.len() {
                break;
            }
            let chunk_offset = u64::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            offsets.push(chunk_offset);
        }

        builder.set_chunk_offsets(offsets);
        Ok(())
    }

    /// Parse ctts (composition time to sample).
    fn parse_ctts(&mut self, atom: &Atom, builder: &mut SampleTableBuilder) -> Result<()> {
        let data = self.read_atom_data(atom)?;

        if data.len() < 8 {
            return Ok(());
        }

        let version = data[0];
        let entry_count = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let mut entries = Vec::with_capacity(entry_count);

        for i in 0..entry_count {
            let offset = 8 + i * 8;
            if offset + 8 > data.len() {
                break;
            }
            let count = u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let cts_offset = if version == 0 {
                u32::from_be_bytes([
                    data[offset + 4],
                    data[offset + 5],
                    data[offset + 6],
                    data[offset + 7],
                ]) as i32
            } else {
                i32::from_be_bytes([
                    data[offset + 4],
                    data[offset + 5],
                    data[offset + 6],
                    data[offset + 7],
                ])
            };
            entries.push((count, cts_offset));
        }

        builder.set_ctts(entries);
        Ok(())
    }

    /// Parse stsd (sample description) - extract codec config.
    fn parse_stsd(&mut self, atom: &Atom, track: &mut TrackInfo) -> Result<()> {
        let data = self.read_atom_data(atom)?;

        if data.len() < 16 {
            return Ok(());
        }

        // Skip version/flags (4) and entry count (4)
        // Then parse first sample entry

        // For audio, extract channel count, sample rate, and esds codec config
        if track.handler_type.is_audio() && data.len() >= 44 {
            // AudioSampleEntry layout (after stsd header[8] + box header[8] + SampleEntry[8]):
            // [24..26] version, [26..28] revision, [28..32] vendor
            // [32..34] channelCount, [34..36] sampleSize
            // [36..38] compressionID, [38..40] packetSize
            // [40..44] sampleRate (16.16 fixed-point)
            let channels = u16::from_be_bytes([data[32], data[33]]);
            let sample_rate = u32::from_be_bytes([data[40], data[41], data[42], data[43]]) >> 16;
            track.channels = Some(channels);
            track.sample_rate = Some(sample_rate);

            // Scan child boxes after the fixed AudioSampleEntry fields for esds
            let mut pos = 44;
            while pos + 8 <= data.len() {
                let box_size = u32::from_be_bytes([
                    data[pos],
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                ]) as usize;
                let box_type = &data[pos + 4..pos + 8];

                if box_size < 8 || pos + box_size > data.len() {
                    break;
                }

                if box_type == b"esds" {
                    // esds is a FullBox: include version/flags after box header
                    if box_size > 12 {
                        let config_data = data[pos + 8..pos + box_size].to_vec();
                        track.codec_data = Some(config_data);
                    }
                    break;
                }

                pos += box_size;
            }
        }

        // Extract codec configuration data for video tracks
        if track.handler_type.is_video() && data.len() > 94 {
            // Scan child boxes after the sample entry header
            let mut pos = 94;
            while pos + 8 <= data.len() {
                let box_size = u32::from_be_bytes([
                    data[pos],
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                ]) as usize;
                let box_type = &data[pos + 4..pos + 8];

                if box_size < 8 || pos + box_size > data.len() {
                    break;
                }

                // Extract avcC or hvcC box contents (excluding the box header)
                if box_type == b"avcC" || box_type == b"hvcC" {
                    let config_data = data[pos + 8..pos + box_size].to_vec();
                    track.codec_data = Some(config_data);
                    break;
                }

                pos += box_size;
            }
        }

        Ok(())
    }
}
