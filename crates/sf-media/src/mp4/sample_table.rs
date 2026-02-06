//! Parse MP4 sample table atoms (stts, ctts, stss, stsz, stsc, stco, co64)
//! and resolve into a flat list of samples with absolute file offsets.

use std::io::{self, Read, Seek, SeekFrom};

use super::atoms::{find_child_box, read_fullbox_header, read_i32, read_u32, read_u64};

/// A fully resolved sample with absolute file position.
#[derive(Debug, Clone)]
pub struct ResolvedSample {
    pub index: u32,
    pub file_offset: u64,
    pub size: u32,
    pub duration: u32,
    pub composition_offset: i32,
    pub is_sync: bool,
    pub decode_timestamp: u64,
}

/// A resolved sample table for one track.
#[derive(Debug, Clone)]
pub struct ResolvedSampleTable {
    pub samples: Vec<ResolvedSample>,
    pub timescale: u32,
}

/// (count, delta) pair from stts.
struct SttsEntry {
    count: u32,
    delta: u32,
}

/// (count, offset) pair from ctts.
struct CttsEntry {
    count: u32,
    offset: i32,
}

/// (first_chunk, samples_per_chunk, sample_description_index) from stsc.
struct StscEntry {
    first_chunk: u32,
    samples_per_chunk: u32,
}

/// Parse the stts atom (decoding time-to-sample). Returns (count, delta) pairs.
fn parse_stts<R: Read>(reader: &mut R) -> io::Result<Vec<SttsEntry>> {
    let (_version, _flags) = read_fullbox_header(reader)?;
    let entry_count = read_u32(reader)?;
    let mut entries = Vec::with_capacity(entry_count as usize);
    for _ in 0..entry_count {
        let count = read_u32(reader)?;
        let delta = read_u32(reader)?;
        entries.push(SttsEntry { count, delta });
    }
    Ok(entries)
}

/// Parse the ctts atom (composition time offsets). Returns (count, offset) pairs.
fn parse_ctts<R: Read>(reader: &mut R) -> io::Result<Vec<CttsEntry>> {
    let (version, _flags) = read_fullbox_header(reader)?;
    let entry_count = read_u32(reader)?;
    let mut entries = Vec::with_capacity(entry_count as usize);
    for _ in 0..entry_count {
        let count = read_u32(reader)?;
        let offset = if version == 0 {
            read_u32(reader)? as i32
        } else {
            read_i32(reader)?
        };
        entries.push(CttsEntry { count, offset });
    }
    Ok(entries)
}

/// Parse the stss atom (sync sample table). Returns set of 0-based sync sample indices.
fn parse_stss<R: Read>(reader: &mut R) -> io::Result<Vec<u32>> {
    let (_version, _flags) = read_fullbox_header(reader)?;
    let entry_count = read_u32(reader)?;
    let mut syncs = Vec::with_capacity(entry_count as usize);
    for _ in 0..entry_count {
        let sample_number = read_u32(reader)?;
        // stss uses 1-based sample numbers.
        syncs.push(sample_number - 1);
    }
    Ok(syncs)
}

/// Parse the stsz atom (sample sizes). Returns per-sample sizes.
fn parse_stsz<R: Read>(reader: &mut R) -> io::Result<Vec<u32>> {
    let (_version, _flags) = read_fullbox_header(reader)?;
    let sample_size = read_u32(reader)?;
    let sample_count = read_u32(reader)?;
    if sample_size != 0 {
        // Fixed-size samples.
        Ok(vec![sample_size; sample_count as usize])
    } else {
        let mut sizes = Vec::with_capacity(sample_count as usize);
        for _ in 0..sample_count {
            sizes.push(read_u32(reader)?);
        }
        Ok(sizes)
    }
}

/// Parse the stsc atom (sample-to-chunk).
fn parse_stsc<R: Read>(reader: &mut R) -> io::Result<Vec<StscEntry>> {
    let (_version, _flags) = read_fullbox_header(reader)?;
    let entry_count = read_u32(reader)?;
    let mut entries = Vec::with_capacity(entry_count as usize);
    for _ in 0..entry_count {
        let first_chunk = read_u32(reader)?;
        let samples_per_chunk = read_u32(reader)?;
        let _sdi = read_u32(reader)?; // sample description index
        entries.push(StscEntry {
            first_chunk,
            samples_per_chunk,
        });
    }
    Ok(entries)
}

/// Parse chunk offsets from stco (32-bit) or co64 (64-bit).
fn parse_chunk_offsets<R: Read>(reader: &mut R, is_co64: bool) -> io::Result<Vec<u64>> {
    let (_version, _flags) = read_fullbox_header(reader)?;
    let entry_count = read_u32(reader)?;
    let mut offsets = Vec::with_capacity(entry_count as usize);
    for _ in 0..entry_count {
        if is_co64 {
            offsets.push(read_u64(reader)?);
        } else {
            offsets.push(read_u32(reader)? as u64);
        }
    }
    Ok(offsets)
}

/// Resolve the sample table from the raw stbl box content.
///
/// `stbl_content_size` is the number of content bytes inside the stbl box.
/// The reader should be positioned at the start of stbl's content.
pub fn resolve_sample_table<R: Read + Seek>(
    reader: &mut R,
    stbl_content_size: u64,
    timescale: u32,
) -> io::Result<ResolvedSampleTable> {
    let stbl_start = reader.stream_position()?;

    // --- Parse stts ---
    reader.seek(SeekFrom::Start(stbl_start))?;
    let stts = find_child_box(reader, stbl_content_size, b"stts")?
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing stts box"))?;
    let stts_entries = parse_stts(reader)?;
    let _ = stts;

    // --- Parse ctts (optional) ---
    reader.seek(SeekFrom::Start(stbl_start))?;
    let ctts_entries = if let Some(_) = find_child_box(reader, stbl_content_size, b"ctts")? {
        parse_ctts(reader)?
    } else {
        Vec::new()
    };

    // --- Parse stss (optional — if absent, all samples are sync) ---
    reader.seek(SeekFrom::Start(stbl_start))?;
    let sync_set: Option<Vec<u32>> =
        if let Some(_) = find_child_box(reader, stbl_content_size, b"stss")? {
            Some(parse_stss(reader)?)
        } else {
            None
        };

    // --- Parse stsz ---
    reader.seek(SeekFrom::Start(stbl_start))?;
    let sizes = if let Some(_) = find_child_box(reader, stbl_content_size, b"stsz")? {
        parse_stsz(reader)?
    } else {
        // Try compact stsz (stz2) — just treat as error for now.
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Missing stsz box",
        ));
    };

    // --- Parse stsc ---
    reader.seek(SeekFrom::Start(stbl_start))?;
    let stsc_entries = find_child_box(reader, stbl_content_size, b"stsc")?
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing stsc box"))
        .and_then(|_| parse_stsc(reader))?;

    // --- Parse stco or co64 ---
    reader.seek(SeekFrom::Start(stbl_start))?;
    let chunk_offsets = if let Some(_) = find_child_box(reader, stbl_content_size, b"stco")? {
        parse_chunk_offsets(reader, false)?
    } else {
        reader.seek(SeekFrom::Start(stbl_start))?;
        if let Some(_) = find_child_box(reader, stbl_content_size, b"co64")? {
            parse_chunk_offsets(reader, true)?
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Missing stco/co64 box",
            ));
        }
    };

    // --- Resolve: map each sample to its file offset ---
    let sample_count = sizes.len();
    let mut samples = Vec::with_capacity(sample_count);

    // Build per-chunk sample count from stsc.
    // stsc entries use 1-based chunk indices.
    let total_chunks = chunk_offsets.len() as u32;
    let mut chunk_sample_counts = Vec::with_capacity(total_chunks as usize);
    for chunk_idx in 0..total_chunks {
        let chunk_number = chunk_idx + 1; // 1-based
        // Find which stsc entry applies (last entry whose first_chunk <= chunk_number).
        let mut spc = stsc_entries[0].samples_per_chunk;
        for e in &stsc_entries {
            if e.first_chunk <= chunk_number {
                spc = e.samples_per_chunk;
            } else {
                break;
            }
        }
        chunk_sample_counts.push(spc);
    }

    // Walk chunks, assigning file offsets to samples.
    let mut sample_idx = 0u32;
    for (chunk_idx, &chunk_offset) in chunk_offsets.iter().enumerate() {
        let spc = chunk_sample_counts[chunk_idx];
        let mut offset = chunk_offset;
        for _ in 0..spc {
            if (sample_idx as usize) >= sample_count {
                break;
            }
            let size = sizes[sample_idx as usize];
            samples.push((sample_idx, offset, size));
            offset += size as u64;
            sample_idx += 1;
        }
    }

    // --- Assign DTS from stts ---
    let mut dts_values = Vec::with_capacity(sample_count);
    let mut dts: u64 = 0;
    for entry in &stts_entries {
        for _ in 0..entry.count {
            dts_values.push((dts, entry.delta));
            dts += entry.delta as u64;
        }
    }

    // --- Assign CTS offsets from ctts ---
    let mut cts_offsets = vec![0i32; sample_count];
    {
        let mut i = 0usize;
        for entry in &ctts_entries {
            for _ in 0..entry.count {
                if i < sample_count {
                    cts_offsets[i] = entry.offset;
                    i += 1;
                }
            }
        }
    }

    // --- Build sync set for O(1) lookup ---
    let sync_set_hash: Option<std::collections::HashSet<u32>> =
        sync_set.map(|v| v.into_iter().collect());

    // --- Assemble final samples ---
    let mut resolved = Vec::with_capacity(sample_count);
    for &(idx, file_offset, size) in &samples {
        let i = idx as usize;
        let (decode_ts, duration) = if i < dts_values.len() {
            dts_values[i]
        } else {
            (0, 0)
        };
        let composition_offset = cts_offsets.get(i).copied().unwrap_or(0);
        let is_sync = match &sync_set_hash {
            Some(set) => set.contains(&idx),
            None => true, // No stss means all samples are sync.
        };

        resolved.push(ResolvedSample {
            index: idx,
            file_offset,
            size,
            duration,
            composition_offset,
            is_sync,
            decode_timestamp: decode_ts,
        });
    }

    Ok(ResolvedSampleTable {
        samples: resolved,
        timescale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fmp4::boxes;
    use std::io::Cursor;

    /// Helper: build a minimal stbl with known sample data and parse it back.
    #[test]
    fn test_resolve_sample_table_roundtrip() {
        // We'll create a minimal stbl manually.
        let mut stbl_content = Vec::new();

        // stsd (minimal, just to have something)
        let stsd = {
            let mut c = Vec::new();
            c.extend_from_slice(&boxes::fullbox_header(0, 0));
            c.extend_from_slice(&0u32.to_be_bytes()); // entry count = 0
            boxes::write_box(b"stsd", &c)
        };
        stbl_content.extend_from_slice(&stsd);

        // stts: 4 samples, each with duration 1000
        let stts = {
            let mut c = Vec::new();
            c.extend_from_slice(&boxes::fullbox_header(0, 0));
            c.extend_from_slice(&1u32.to_be_bytes()); // 1 entry
            c.extend_from_slice(&4u32.to_be_bytes()); // count=4
            c.extend_from_slice(&1000u32.to_be_bytes()); // delta=1000
            boxes::write_box(b"stts", &c)
        };
        stbl_content.extend_from_slice(&stts);

        // stss: samples 1, 3 are sync (1-based)
        let stss = {
            let mut c = Vec::new();
            c.extend_from_slice(&boxes::fullbox_header(0, 0));
            c.extend_from_slice(&2u32.to_be_bytes()); // 2 entries
            c.extend_from_slice(&1u32.to_be_bytes()); // sample 1
            c.extend_from_slice(&3u32.to_be_bytes()); // sample 3
            boxes::write_box(b"stss", &c)
        };
        stbl_content.extend_from_slice(&stss);

        // stsz: 4 samples with sizes [100, 50, 200, 75]
        let stsz = {
            let mut c = Vec::new();
            c.extend_from_slice(&boxes::fullbox_header(0, 0));
            c.extend_from_slice(&0u32.to_be_bytes()); // sample_size=0 (variable)
            c.extend_from_slice(&4u32.to_be_bytes()); // count=4
            c.extend_from_slice(&100u32.to_be_bytes());
            c.extend_from_slice(&50u32.to_be_bytes());
            c.extend_from_slice(&200u32.to_be_bytes());
            c.extend_from_slice(&75u32.to_be_bytes());
            boxes::write_box(b"stsz", &c)
        };
        stbl_content.extend_from_slice(&stsz);

        // stsc: 1 entry, first_chunk=1, samples_per_chunk=2
        // (2 chunks, 2 samples each)
        let stsc = {
            let mut c = Vec::new();
            c.extend_from_slice(&boxes::fullbox_header(0, 0));
            c.extend_from_slice(&1u32.to_be_bytes()); // 1 entry
            c.extend_from_slice(&1u32.to_be_bytes()); // first_chunk=1
            c.extend_from_slice(&2u32.to_be_bytes()); // samples_per_chunk=2
            c.extend_from_slice(&1u32.to_be_bytes()); // sample_description_index=1
            boxes::write_box(b"stsc", &c)
        };
        stbl_content.extend_from_slice(&stsc);

        // stco: 2 chunks at offsets [1000, 2000]
        let stco = {
            let mut c = Vec::new();
            c.extend_from_slice(&boxes::fullbox_header(0, 0));
            c.extend_from_slice(&2u32.to_be_bytes()); // 2 entries
            c.extend_from_slice(&1000u32.to_be_bytes());
            c.extend_from_slice(&2000u32.to_be_bytes());
            boxes::write_box(b"stco", &c)
        };
        stbl_content.extend_from_slice(&stco);

        let mut cursor = Cursor::new(&stbl_content);
        let table = resolve_sample_table(&mut cursor, stbl_content.len() as u64, 90000).unwrap();

        assert_eq!(table.timescale, 90000);
        assert_eq!(table.samples.len(), 4);

        // Check offsets: chunk1 starts at 1000, sample0=100, sample1=50
        assert_eq!(table.samples[0].file_offset, 1000);
        assert_eq!(table.samples[0].size, 100);
        assert_eq!(table.samples[1].file_offset, 1100);
        assert_eq!(table.samples[1].size, 50);

        // Chunk2 starts at 2000, sample2=200, sample3=75
        assert_eq!(table.samples[2].file_offset, 2000);
        assert_eq!(table.samples[2].size, 200);
        assert_eq!(table.samples[3].file_offset, 2200);
        assert_eq!(table.samples[3].size, 75);

        // DTS values
        assert_eq!(table.samples[0].decode_timestamp, 0);
        assert_eq!(table.samples[1].decode_timestamp, 1000);
        assert_eq!(table.samples[2].decode_timestamp, 2000);
        assert_eq!(table.samples[3].decode_timestamp, 3000);

        // Sync
        assert!(table.samples[0].is_sync);
        assert!(!table.samples[1].is_sync);
        assert!(table.samples[2].is_sync);
        assert!(!table.samples[3].is_sync);

        // Duration
        assert_eq!(table.samples[0].duration, 1000);
    }
}
