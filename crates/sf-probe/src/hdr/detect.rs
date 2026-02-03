//! HEVC bitstream analysis for HDR format detection.
//!
//! Parses HEVC NAL units from codec private data to determine whether a video
//! track uses HDR10 (PQ), HLG, or SDR transfer characteristics.

use sf_core::HdrFormat;

use super::dolby_vision;
use crate::types::DvInfo;

/// Result of HDR detection: the HDR format and optional Dolby Vision info.
pub(crate) struct HdrDetection {
    pub format: HdrFormat,
    pub dv_info: Option<DvInfo>,
}

/// Attempt to detect HDR format from HEVC codec private data.
///
/// Extracts NAL units from the data and inspects:
/// - SPS NAL units (type 33) for VUI colour parameters
/// - SEI NAL units (types 39/40) for HDR10+ markers
/// - UNSPEC62 NAL units (type 62) for Dolby Vision RPU
///
/// Returns `None` if detection fails or data is insufficient.
pub(crate) fn detect_hdr_from_hevc(data: &[u8]) -> Option<HdrDetection> {
    let nal_units = extract_nal_units(data);

    let mut transfer_characteristics: Option<u8> = None;
    let mut has_hdr10plus = false;
    let mut dv_info: Option<DvInfo> = None;

    for (nal_type, nal_data) in &nal_units {
        match *nal_type {
            // SPS (Sequence Parameter Set)
            33 => {
                if let Some(tc) = parse_sps_transfer_characteristics(nal_data) {
                    transfer_characteristics = Some(tc);
                }
            }
            // SEI Prefix (39) or Suffix (40)
            39 | 40 => {
                parse_sei_for_hdr10plus(nal_data, &mut has_hdr10plus);
            }
            // Dolby Vision RPU (UNSPEC62)
            62 => {
                if let Some(dv) = dolby_vision::parse_rpu_nal(nal_data) {
                    dv_info = Some(dv);
                }
            }
            _ => {}
        }
    }

    // Dolby Vision takes highest priority.
    if let Some(dv) = dv_info {
        return Some(HdrDetection {
            format: HdrFormat::DolbyVision,
            dv_info: Some(dv),
        });
    }

    if has_hdr10plus {
        return Some(HdrDetection {
            format: HdrFormat::Hdr10Plus,
            dv_info: None,
        });
    }

    match transfer_characteristics {
        // SMPTE ST 2084 (PQ) = HDR10.
        Some(16) => Some(HdrDetection {
            format: HdrFormat::Hdr10,
            dv_info: None,
        }),
        // ARIB STD-B67 = HLG.
        Some(18) => Some(HdrDetection {
            format: HdrFormat::Hlg,
            dv_info: None,
        }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// NAL unit extraction
// ---------------------------------------------------------------------------

/// Extract HEVC NAL units from byte data.
///
/// Handles both Annex B start-code format (0x00 0x00 0x01 / 0x00 0x00 0x00 0x01)
/// and length-prefixed (HVCC) format.
fn extract_nal_units(data: &[u8]) -> Vec<(u8, Vec<u8>)> {
    let mut nals = extract_annex_b(data);
    if nals.is_empty() && data.len() > 4 {
        nals = extract_length_prefixed(data);
    }
    nals
}

fn extract_annex_b(data: &[u8]) -> Vec<(u8, Vec<u8>)> {
    let mut result = Vec::new();
    let mut i = 0;
    let mut last_start: Option<usize> = None;

    while i < data.len() {
        let is_start = if i + 3 < data.len() {
            (data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1)
                || (data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 0 && data[i + 3] == 1)
        } else if i + 2 < data.len() {
            data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1
        } else {
            false
        };

        if is_start {
            if let Some(start) = last_start {
                push_nal(&data[start..i], &mut result);
            }
            if i + 2 < data.len() && data[i + 2] == 1 {
                i += 3;
            } else {
                i += 4;
            }
            last_start = Some(i);
        } else {
            i += 1;
        }
    }

    // Last NAL unit.
    if let Some(start) = last_start {
        if start < data.len() {
            push_nal(&data[start..], &mut result);
        }
    }

    result
}

fn extract_length_prefixed(data: &[u8]) -> Vec<(u8, Vec<u8>)> {
    let mut result = Vec::new();
    let mut i = 0;

    while i + 4 <= data.len() {
        let length =
            u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize;
        i += 4;

        if length == 0 || i + length > data.len() {
            break;
        }

        push_nal(&data[i..i + length], &mut result);
        i += length;
    }

    result
}

fn push_nal(nal_data: &[u8], out: &mut Vec<(u8, Vec<u8>)>) {
    if !nal_data.is_empty() {
        // HEVC NAL header: 2 bytes. NAL type = bits [1..6] of first byte.
        let nal_type = (nal_data[0] >> 1) & 0x3F;
        out.push((nal_type, nal_data.to_vec()));
    }
}

// ---------------------------------------------------------------------------
// SPS parsing (simplified)
// ---------------------------------------------------------------------------

/// Attempt to extract transfer_characteristics from an SPS NAL unit's VUI section.
///
/// Uses a byte-scanning heuristic: look for the pattern
///   colour_primaries=9 (BT.2020)  followed by  transfer_characteristics=16 (PQ) or 18 (HLG)
/// at byte-aligned positions.
fn parse_sps_transfer_characteristics(sps_data: &[u8]) -> Option<u8> {
    if sps_data.len() < 3 {
        return None;
    }

    // Skip the 2-byte NAL header.
    let payload = &sps_data[2..];

    for i in 0..payload.len().saturating_sub(2) {
        // BT.2020 primaries (9) followed by PQ (16) or HLG (18).
        if payload[i] == 9 && (payload[i + 1] == 16 || payload[i + 1] == 18) {
            return Some(payload[i + 1]);
        }
        // BT.709 primaries (1) with PQ transfer (16).
        if payload[i] == 1 && payload[i + 1] == 16 {
            return Some(payload[i + 1]);
        }
    }

    None
}

// ---------------------------------------------------------------------------
// SEI parsing (HDR10+ detection)
// ---------------------------------------------------------------------------

/// Scan SEI payload for HDR10+ user-data-registered marker.
fn parse_sei_for_hdr10plus(sei_data: &[u8], has_hdr10plus: &mut bool) {
    if sei_data.len() < 3 {
        return;
    }

    let mut i = 2; // skip NAL header

    while i < sei_data.len() {
        // Read payload type.
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

        // Read payload size.
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

        // user_data_registered_itu_t_t35 (type 4) containing HDR10+ signature.
        if payload_type == 4 && is_hdr10plus_sei(payload) {
            *has_hdr10plus = true;
        }

        i += payload_size;
    }
}

/// Check for Samsung HDR10+ ITU-T T.35 signature.
fn is_hdr10plus_sei(payload: &[u8]) -> bool {
    payload.len() >= 5
        && payload[0] == 0xB5
        && payload[1] == 0x00
        && payload[2] == 0x3C
        && payload[3] == 0x00
        && payload[4] == 0x01
}
