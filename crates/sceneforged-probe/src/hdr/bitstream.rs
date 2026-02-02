//! HDR detection from video bitstream (HEVC)

use crate::hdr::dolby_vision;
use crate::types::{ContentLightLevel, HdrFormat, MasteringDisplay};

/// Detect HDR format from HEVC bitstream data
///
/// This parses HEVC NAL units to extract:
/// - VUI parameters from SPS (transfer characteristics)
/// - SEI messages (HDR10 static metadata, HDR10+ dynamic metadata)
/// - Dolby Vision RPU NAL units
pub fn detect_hdr_from_hevc(data: &[u8]) -> Option<HdrFormat> {
    let nal_units = extract_nal_units(data);

    let mut transfer_characteristics = None;
    let mut mastering_display = None;
    let mut content_light_level = None;
    let mut has_hdr10plus = false;
    let mut dv_info = None;

    for (nal_type, nal_data) in &nal_units {
        match *nal_type {
            // SPS (Sequence Parameter Set)
            33 => {
                if let Some(tc) = parse_sps_transfer_characteristics(nal_data) {
                    transfer_characteristics = Some(tc);
                }
            }
            // SEI (Supplemental Enhancement Information)
            39 | 40 => {
                // Prefix SEI = 39, Suffix SEI = 40
                parse_sei_messages(
                    nal_data,
                    &mut mastering_display,
                    &mut content_light_level,
                    &mut has_hdr10plus,
                );
            }
            // Dolby Vision RPU (NAL type 62 in HEVC, or unspec62)
            62 => {
                if let Some(dv) = dolby_vision::parse_rpu(nal_data) {
                    dv_info = Some(dv);
                }
            }
            _ => {}
        }
    }

    // Determine HDR format based on detected characteristics
    if let Some(dv) = dv_info {
        return Some(dv);
    }

    if has_hdr10plus {
        return Some(HdrFormat::Hdr10Plus {
            mastering_display,
            content_light_level,
        });
    }

    match transfer_characteristics {
        Some(16) => {
            // SMPTE ST 2084 (PQ) = HDR10
            Some(HdrFormat::Hdr10 {
                mastering_display,
                content_light_level,
            })
        }
        Some(18) => {
            // ARIB STD-B67 (HLG)
            Some(HdrFormat::Hlg)
        }
        _ => None,
    }
}

/// Extract NAL units from byte stream
///
/// Handles both Annex B format (start codes) and length-prefixed format
fn extract_nal_units(data: &[u8]) -> Vec<(u8, Vec<u8>)> {
    let mut nal_units = Vec::new();

    // Try Annex B format first (0x00 0x00 0x01 or 0x00 0x00 0x00 0x01)
    let mut i = 0;
    let mut last_nal_start = None;

    while i < data.len() {
        // Look for start codes
        let is_start_code = if i + 3 < data.len() {
            (data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1)
                || (data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 0 && data[i + 3] == 1)
        } else if i + 2 < data.len() {
            data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1
        } else {
            false
        };

        if is_start_code {
            // Save previous NAL unit
            if let Some(start) = last_nal_start {
                let nal_data = &data[start..i];
                if !nal_data.is_empty() {
                    // HEVC NAL header is 2 bytes, NAL type is bits 1-6 of first byte
                    let nal_type = (nal_data[0] >> 1) & 0x3F;
                    nal_units.push((nal_type, nal_data.to_vec()));
                }
            }

            // Skip start code
            if data[i + 2] == 1 {
                i += 3;
            } else {
                i += 4;
            }
            last_nal_start = Some(i);
        } else {
            i += 1;
        }
    }

    // Don't forget the last NAL unit
    if let Some(start) = last_nal_start {
        if start < data.len() {
            let nal_data = &data[start..];
            if !nal_data.is_empty() {
                let nal_type = (nal_data[0] >> 1) & 0x3F;
                nal_units.push((nal_type, nal_data.to_vec()));
            }
        }
    }

    // If no Annex B NAL units found, try length-prefixed format (AVCC/HVCC)
    if nal_units.is_empty() && data.len() > 4 {
        nal_units = extract_length_prefixed_nals(data);
    }

    nal_units
}

/// Extract NAL units from length-prefixed format (HVCC)
fn extract_length_prefixed_nals(data: &[u8]) -> Vec<(u8, Vec<u8>)> {
    let mut nal_units = Vec::new();
    let mut i = 0;

    // Common NAL length sizes are 4 bytes
    let nal_length_size = 4;

    while i + nal_length_size <= data.len() {
        let length = match nal_length_size {
            4 => u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize,
            2 => u16::from_be_bytes([data[i], data[i + 1]]) as usize,
            1 => data[i] as usize,
            _ => break,
        };

        i += nal_length_size;

        if i + length > data.len() || length == 0 {
            break;
        }

        let nal_data = &data[i..i + length];
        if !nal_data.is_empty() {
            let nal_type = (nal_data[0] >> 1) & 0x3F;
            nal_units.push((nal_type, nal_data.to_vec()));
        }

        i += length;
    }

    nal_units
}

/// Parse SPS to extract transfer characteristics from VUI
fn parse_sps_transfer_characteristics(sps_data: &[u8]) -> Option<u8> {
    // Skip NAL header (2 bytes for HEVC)
    if sps_data.len() < 3 {
        return None;
    }

    // SPS parsing is complex due to exp-golomb coding
    // For a quick check, we look for common patterns in the VUI section

    // The VUI parameters are near the end of SPS
    // We use a simplified heuristic: scan for colour_description_present_flag pattern

    // In a proper implementation, we'd fully parse the SPS using bitstream-io
    // For now, use a pattern-matching approach for common cases

    let reader = SimpleBitReader::new(&sps_data[2..]); // Skip NAL header

    // Try to find VUI parameters
    // This is a simplified parser that looks for common VUI patterns
    parse_sps_vui_simple(reader)
}

/// Simplified SPS VUI parser
fn parse_sps_vui_simple(reader: SimpleBitReader) -> Option<u8> {
    // Skip to VUI section - this is highly simplified
    // A full implementation would parse all SPS fields

    // Look for a sequence that could be VUI colour description
    // colour_primaries (8 bits) + transfer_characteristics (8 bits) + matrix_coeffs (8 bits)
    // preceded by colour_description_present_flag (1 bit) = 1

    // Scan through data looking for reasonable VUI values
    let data = reader.data;

    for i in 0..data.len().saturating_sub(3) {
        // Look for byte-aligned colour description
        // If we find BT.2020 primaries (9) followed by PQ (16) or HLG (18)
        if data[i] == 9 && (data[i + 1] == 16 || data[i + 1] == 18) {
            return Some(data[i + 1]);
        }
        // BT.709 primaries (1) with PQ transfer (16)
        if data[i] == 1 && data[i + 1] == 16 {
            return Some(data[i + 1]);
        }
    }

    None
}

/// Parse SEI messages for HDR metadata
fn parse_sei_messages(
    sei_data: &[u8],
    mastering_display: &mut Option<MasteringDisplay>,
    content_light_level: &mut Option<ContentLightLevel>,
    has_hdr10plus: &mut bool,
) {
    // Skip NAL header (2 bytes for HEVC)
    if sei_data.len() < 3 {
        return;
    }

    let mut i = 2; // Skip NAL header

    while i < sei_data.len() {
        // Read payload type
        let mut payload_type = 0u32;
        while i < sei_data.len() && sei_data[i] == 0xFF {
            payload_type += 255;
            i += 1;
        }
        if i >= sei_data.len() {
            break;
        }
        payload_type += sei_data[i] as u32;
        i += 1;

        // Read payload size
        let mut payload_size = 0usize;
        while i < sei_data.len() && sei_data[i] == 0xFF {
            payload_size += 255;
            i += 1;
        }
        if i >= sei_data.len() {
            break;
        }
        payload_size += sei_data[i] as usize;
        i += 1;

        if i + payload_size > sei_data.len() {
            break;
        }

        let payload = &sei_data[i..i + payload_size];

        match payload_type {
            // Mastering Display Colour Volume (SEI type 137)
            137 => {
                if let Some(md) = parse_mastering_display_sei(payload) {
                    *mastering_display = Some(md);
                }
            }
            // Content Light Level Info (SEI type 144)
            144 => {
                if let Some(cll) = parse_content_light_level_sei(payload) {
                    *content_light_level = Some(cll);
                }
            }
            // User data registered (may contain HDR10+)
            4 => {
                if is_hdr10plus_sei(payload) {
                    *has_hdr10plus = true;
                }
            }
            _ => {}
        }

        i += payload_size;
    }
}

/// Parse Mastering Display Colour Volume SEI
fn parse_mastering_display_sei(payload: &[u8]) -> Option<MasteringDisplay> {
    // SMPTE ST 2086 mastering display metadata
    // 24 bytes: 3x2 primaries (6x2 bytes) + white point (2x2 bytes) + max/min luma (4+4 bytes)
    if payload.len() < 24 {
        return None;
    }

    let primaries = [
        // Green
        [
            u16::from_be_bytes([payload[0], payload[1]]),
            u16::from_be_bytes([payload[2], payload[3]]),
        ],
        // Blue
        [
            u16::from_be_bytes([payload[4], payload[5]]),
            u16::from_be_bytes([payload[6], payload[7]]),
        ],
        // Red
        [
            u16::from_be_bytes([payload[8], payload[9]]),
            u16::from_be_bytes([payload[10], payload[11]]),
        ],
    ];

    let white_point = [
        u16::from_be_bytes([payload[12], payload[13]]),
        u16::from_be_bytes([payload[14], payload[15]]),
    ];

    let max_luminance = u32::from_be_bytes([payload[16], payload[17], payload[18], payload[19]]);
    let min_luminance = u32::from_be_bytes([payload[20], payload[21], payload[22], payload[23]]);

    Some(MasteringDisplay {
        primaries,
        white_point,
        max_luminance,
        min_luminance,
    })
}

/// Parse Content Light Level SEI
fn parse_content_light_level_sei(payload: &[u8]) -> Option<ContentLightLevel> {
    if payload.len() < 4 {
        return None;
    }

    let max_cll = u16::from_be_bytes([payload[0], payload[1]]);
    let max_fall = u16::from_be_bytes([payload[2], payload[3]]);

    Some(ContentLightLevel { max_cll, max_fall })
}

/// Check if SEI payload is HDR10+ metadata
fn is_hdr10plus_sei(payload: &[u8]) -> bool {
    // HDR10+ uses user_data_registered_itu_t_t35
    // ITU-T T.35 country code: 0xB5 (USA)
    // Terminal provider code: 0x003C (Samsung)
    // Application identifier: 0x0001 (HDR10+)
    if payload.len() < 7 {
        return false;
    }

    // Check for Samsung HDR10+ signature
    payload[0] == 0xB5
        && payload[1] == 0x00
        && payload[2] == 0x3C
        && payload[3] == 0x00
        && payload[4] == 0x01
}

/// Simple bit reader for parsing
struct SimpleBitReader<'a> {
    data: &'a [u8],
    #[allow(dead_code)]
    byte_pos: usize,
    #[allow(dead_code)]
    bit_pos: u8,
}

impl<'a> SimpleBitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }
}
