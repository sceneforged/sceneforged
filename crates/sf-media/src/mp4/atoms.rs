//! ISO BMFF box header parsing and navigation.

use std::io::{self, Read, Seek, SeekFrom};

/// A parsed box header.
#[derive(Debug, Clone)]
pub struct BoxHeader {
    /// 4-byte box type (e.g. b"moov").
    pub box_type: [u8; 4],
    /// Total size of the box including the header.
    pub size: u64,
    /// Size of the header itself (8 or 16 for extended-size boxes).
    pub header_size: u64,
}

impl BoxHeader {
    /// Size of the box content (size - header_size).
    pub fn content_size(&self) -> u64 {
        self.size.saturating_sub(self.header_size)
    }
}

/// Read a box header from the current position.
///
/// Returns `Ok(None)` at EOF, `Ok(Some(header))` otherwise.
pub fn read_box_header<R: Read>(reader: &mut R) -> io::Result<Option<BoxHeader>> {
    let mut buf = [0u8; 8];
    match reader.read_exact(&mut buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }

    let size32 = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let box_type = [buf[4], buf[5], buf[6], buf[7]];

    let (size, header_size) = if size32 == 1 {
        // Extended 64-bit size.
        let mut ext = [0u8; 8];
        reader.read_exact(&mut ext)?;
        let size64 = u64::from_be_bytes(ext);
        (size64, 16u64)
    } else if size32 == 0 {
        // Box extends to end of file — we can't determine size without seeking.
        // Return 0 as a sentinel; callers that need the real size must handle this.
        (0u64, 8u64)
    } else {
        (size32 as u64, 8u64)
    };

    Ok(Some(BoxHeader {
        box_type,
        size,
        header_size,
    }))
}

/// Skip past the current box's remaining content.
pub fn skip_box<R: Read + Seek>(reader: &mut R, header: &BoxHeader) -> io::Result<()> {
    let content_size = header.content_size();
    reader.seek(SeekFrom::Current(content_size as i64))?;
    Ok(())
}

/// Search for a child box with the given type within `parent_content_size` bytes.
///
/// If found, the reader is positioned at the start of the child's content
/// (just past its header). Returns the child's header.
pub fn find_child_box<R: Read + Seek>(
    reader: &mut R,
    parent_content_size: u64,
    target: &[u8; 4],
) -> io::Result<Option<BoxHeader>> {
    let start = reader.stream_position()?;
    let end = start + parent_content_size;

    while reader.stream_position()? < end {
        let Some(header) = read_box_header(reader)? else {
            return Ok(None);
        };
        if header.size == 0 {
            return Ok(None);
        }
        if &header.box_type == target {
            return Ok(Some(header));
        }
        // Skip this box's content.
        let content_size = header.content_size();
        reader.seek(SeekFrom::Current(content_size as i64))?;
    }

    Ok(None)
}

/// Read a big-endian u16.
pub fn read_u16<R: Read>(reader: &mut R) -> io::Result<u16> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf)?;
    Ok(u16::from_be_bytes(buf))
}

/// Read a big-endian u32.
pub fn read_u32<R: Read>(reader: &mut R) -> io::Result<u32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_be_bytes(buf))
}

/// Read a big-endian i32.
pub fn read_i32<R: Read>(reader: &mut R) -> io::Result<i32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(i32::from_be_bytes(buf))
}

/// Read a big-endian u64.
pub fn read_u64<R: Read>(reader: &mut R) -> io::Result<u64> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf)?;
    Ok(u64::from_be_bytes(buf))
}

/// Read `n` bytes into a new Vec.
pub fn read_bytes<R: Read>(reader: &mut R, n: usize) -> io::Result<Vec<u8>> {
    let mut buf = vec![0u8; n];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

/// Read a fullbox header (1 byte version + 3 bytes flags) and return (version, flags).
pub fn read_fullbox_header<R: Read>(reader: &mut R) -> io::Result<(u8, u32)> {
    let val = read_u32(reader)?;
    let version = (val >> 24) as u8;
    let flags = val & 0x00FFFFFF;
    Ok((version, flags))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_read_box_header_normal() {
        let mut data = Vec::new();
        data.extend_from_slice(&100u32.to_be_bytes()); // size
        data.extend_from_slice(b"moov"); // type
        data.extend_from_slice(&[0u8; 92]); // content

        let mut cursor = Cursor::new(&data);
        let header = read_box_header(&mut cursor).unwrap().unwrap();
        assert_eq!(header.box_type, *b"moov");
        assert_eq!(header.size, 100);
        assert_eq!(header.header_size, 8);
        assert_eq!(header.content_size(), 92);
    }

    #[test]
    fn test_read_box_header_extended() {
        let mut data = Vec::new();
        data.extend_from_slice(&1u32.to_be_bytes()); // size=1 -> extended
        data.extend_from_slice(b"mdat");
        data.extend_from_slice(&(5_000_000_000u64).to_be_bytes()); // extended size
        let mut cursor = Cursor::new(&data);
        let header = read_box_header(&mut cursor).unwrap().unwrap();
        assert_eq!(header.size, 5_000_000_000);
        assert_eq!(header.header_size, 16);
    }

    #[test]
    fn test_read_box_header_eof() {
        let data = [0u8; 4]; // too short
        let mut cursor = Cursor::new(&data);
        assert!(read_box_header(&mut cursor).unwrap().is_none());
    }

    #[test]
    fn test_find_child_box() {
        // Build two child boxes: [ftyp(16)] [moov(20)]
        let mut data = Vec::new();
        // ftyp box: size=16, content=8 bytes
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"ftyp");
        data.extend_from_slice(&[0u8; 8]);
        // moov box: size=20, content=12 bytes
        data.extend_from_slice(&20u32.to_be_bytes());
        data.extend_from_slice(b"moov");
        data.extend_from_slice(&[0xAA; 12]);

        let mut cursor = Cursor::new(&data);
        let header = find_child_box(&mut cursor, data.len() as u64, b"moov")
            .unwrap()
            .unwrap();
        assert_eq!(header.box_type, *b"moov");
        assert_eq!(header.content_size(), 12);
    }
}
