//! HEVC NAL unit parsing

/// HEVC NAL unit types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NalUnitType {
    /// Coded slice of trailing picture
    TrailN = 0,
    TrailR = 1,
    /// Coded slice of TSA picture
    TsaN = 2,
    TsaR = 3,
    /// Coded slice of STSA picture
    StsaN = 4,
    StsaR = 5,
    /// Coded slice of RADL picture
    RadlN = 6,
    RadlR = 7,
    /// Coded slice of RASL picture
    RaslN = 8,
    RaslR = 9,
    /// Coded slice of BLA picture
    BlaWLp = 16,
    BlaWRadl = 17,
    BlaNLp = 18,
    /// Coded slice of IDR picture
    IdrWRadl = 19,
    IdrNLp = 20,
    /// Coded slice of CRA picture
    CraNut = 21,
    /// Video Parameter Set
    VpsNut = 32,
    /// Sequence Parameter Set
    SpsNut = 33,
    /// Picture Parameter Set
    PpsNut = 34,
    /// Access Unit Delimiter
    AudNut = 35,
    /// End of Sequence
    EosNut = 36,
    /// End of Bitstream
    EobNut = 37,
    /// Filler Data
    FdNut = 38,
    /// SEI Prefix
    PrefixSeiNut = 39,
    /// SEI Suffix
    SuffixSeiNut = 40,
    /// Dolby Vision RPU
    Unspec62 = 62,
    /// Unknown/other
    Unknown(u8),
}

impl From<u8> for NalUnitType {
    fn from(value: u8) -> Self {
        match value {
            0 => NalUnitType::TrailN,
            1 => NalUnitType::TrailR,
            2 => NalUnitType::TsaN,
            3 => NalUnitType::TsaR,
            4 => NalUnitType::StsaN,
            5 => NalUnitType::StsaR,
            6 => NalUnitType::RadlN,
            7 => NalUnitType::RadlR,
            8 => NalUnitType::RaslN,
            9 => NalUnitType::RaslR,
            16 => NalUnitType::BlaWLp,
            17 => NalUnitType::BlaWRadl,
            18 => NalUnitType::BlaNLp,
            19 => NalUnitType::IdrWRadl,
            20 => NalUnitType::IdrNLp,
            21 => NalUnitType::CraNut,
            32 => NalUnitType::VpsNut,
            33 => NalUnitType::SpsNut,
            34 => NalUnitType::PpsNut,
            35 => NalUnitType::AudNut,
            36 => NalUnitType::EosNut,
            37 => NalUnitType::EobNut,
            38 => NalUnitType::FdNut,
            39 => NalUnitType::PrefixSeiNut,
            40 => NalUnitType::SuffixSeiNut,
            62 => NalUnitType::Unspec62,
            v => NalUnitType::Unknown(v),
        }
    }
}

/// A parsed NAL unit
#[derive(Debug, Clone)]
pub struct NalUnit {
    /// NAL unit type
    pub nal_type: NalUnitType,
    /// Layer ID
    pub nuh_layer_id: u8,
    /// Temporal ID
    pub nuh_temporal_id_plus1: u8,
    /// Raw NAL unit data (without start code, includes header)
    pub data: Vec<u8>,
}

impl NalUnit {
    /// Get the NAL unit payload (data after header)
    pub fn payload(&self) -> &[u8] {
        if self.data.len() > 2 {
            &self.data[2..]
        } else {
            &[]
        }
    }
}

/// Extract NAL units from byte stream
///
/// Handles both Annex B format (start codes) and length-prefixed format
pub fn extract_nal_units(data: &[u8]) -> Vec<NalUnit> {
    // Try Annex B format first
    let units = extract_annex_b(data);
    if !units.is_empty() {
        return units;
    }

    // Try length-prefixed format
    extract_length_prefixed(data, 4)
}

/// Extract NAL units from Annex B byte stream (start code delimited)
fn extract_annex_b(data: &[u8]) -> Vec<NalUnit> {
    let mut units = Vec::new();
    let mut i = 0;

    // Find all start codes and extract NAL units between them
    let mut nal_starts = Vec::new();

    while i < data.len() {
        // Look for 3-byte or 4-byte start codes
        if i + 2 < data.len() && data[i] == 0 && data[i + 1] == 0 {
            if data[i + 2] == 1 {
                // 3-byte start code
                nal_starts.push(i + 3);
                i += 3;
                continue;
            } else if i + 3 < data.len() && data[i + 2] == 0 && data[i + 3] == 1 {
                // 4-byte start code
                nal_starts.push(i + 4);
                i += 4;
                continue;
            }
        }
        i += 1;
    }

    // Extract NAL units between start codes
    for (idx, &start) in nal_starts.iter().enumerate() {
        let end = if idx + 1 < nal_starts.len() {
            // Find the start code before the next NAL
            let next_start = nal_starts[idx + 1];
            // Go back to find the start code (either 3 or 4 bytes)
            if next_start >= 4 && data[next_start - 4..next_start - 1] == [0, 0, 0] {
                next_start - 4
            } else {
                next_start - 3
            }
        } else {
            data.len()
        };

        if start < end && start < data.len() {
            let nal_data = &data[start..end];
            if let Some(unit) = parse_nal_header(nal_data) {
                units.push(unit);
            }
        }
    }

    units
}

/// Extract NAL units from length-prefixed format (HVCC)
fn extract_length_prefixed(data: &[u8], length_size: usize) -> Vec<NalUnit> {
    let mut units = Vec::new();
    let mut i = 0;

    while i + length_size <= data.len() {
        let length = match length_size {
            4 => u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize,
            2 => u16::from_be_bytes([data[i], data[i + 1]]) as usize,
            1 => data[i] as usize,
            _ => break,
        };

        i += length_size;

        if length == 0 || i + length > data.len() {
            break;
        }

        let nal_data = &data[i..i + length];
        if let Some(unit) = parse_nal_header(nal_data) {
            units.push(unit);
        }

        i += length;
    }

    units
}

/// Parse NAL unit header and create NalUnit
fn parse_nal_header(data: &[u8]) -> Option<NalUnit> {
    if data.len() < 2 {
        return None;
    }

    // HEVC NAL unit header (2 bytes):
    // forbidden_zero_bit (1 bit) - must be 0
    // nal_unit_type (6 bits)
    // nuh_layer_id (6 bits)
    // nuh_temporal_id_plus1 (3 bits)

    let forbidden_bit = (data[0] >> 7) & 0x01;
    if forbidden_bit != 0 {
        return None;
    }

    let nal_unit_type = (data[0] >> 1) & 0x3F;
    let nuh_layer_id = ((data[0] & 0x01) << 5) | ((data[1] >> 3) & 0x1F);
    let nuh_temporal_id_plus1 = data[1] & 0x07;

    Some(NalUnit {
        nal_type: NalUnitType::from(nal_unit_type),
        nuh_layer_id,
        nuh_temporal_id_plus1,
        data: data.to_vec(),
    })
}

/// Remove emulation prevention bytes (0x03) from NAL unit payload
///
/// In HEVC, the byte sequence 0x00 0x00 0x03 is used to prevent
/// start code emulation. This function removes the 0x03 bytes.
pub fn remove_emulation_prevention(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;

    while i < data.len() {
        if i + 2 < data.len() && data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 3 {
            // Found emulation prevention byte
            result.push(0);
            result.push(0);
            i += 3; // Skip the 0x03
        } else {
            result.push(data[i]);
            i += 1;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nal_type_from_u8() {
        assert_eq!(NalUnitType::from(33), NalUnitType::SpsNut);
        assert_eq!(NalUnitType::from(39), NalUnitType::PrefixSeiNut);
        assert_eq!(NalUnitType::from(62), NalUnitType::Unspec62);
    }

    #[test]
    fn test_remove_emulation_prevention() {
        let input = vec![0x00, 0x00, 0x03, 0x01, 0x00, 0x00, 0x03, 0x02];
        let output = remove_emulation_prevention(&input);
        assert_eq!(output, vec![0x00, 0x00, 0x01, 0x00, 0x00, 0x02]);
    }
}
