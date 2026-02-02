//! HEVC SEI (Supplemental Enhancement Information) parsing

use super::nal::remove_emulation_prevention;
use crate::types::{ContentLightLevel, MasteringDisplay};

/// SEI message types
#[derive(Debug, Clone)]
pub enum SeiMessage {
    /// Mastering Display Colour Volume (type 137)
    MasteringDisplayColourVolume(MasteringDisplay),
    /// Content Light Level Info (type 144)
    ContentLightLevelInfo(ContentLightLevel),
    /// HDR10+ dynamic metadata (type 4 with specific payload)
    Hdr10Plus,
    /// Unknown or unhandled SEI message
    Unknown(u32),
}

/// SEI payload types we care about
const SEI_TYPE_USER_DATA_REGISTERED: u32 = 4;
const SEI_TYPE_MASTERING_DISPLAY_COLOUR_VOLUME: u32 = 137;
const SEI_TYPE_CONTENT_LIGHT_LEVEL_INFO: u32 = 144;

/// Parse SEI NAL unit and extract messages
pub fn parse_sei(data: &[u8]) -> Vec<SeiMessage> {
    let mut messages = Vec::new();

    if data.len() < 3 {
        return messages;
    }

    // Remove emulation prevention bytes
    let rbsp = remove_emulation_prevention(data);

    // Skip NAL header (2 bytes)
    let mut pos = 2;

    while pos < rbsp.len() - 1 {
        // Don't parse if we hit the rbsp_trailing_bits
        if rbsp[pos] == 0x80 {
            break;
        }

        // Read payload type
        let mut payload_type = 0u32;
        while pos < rbsp.len() && rbsp[pos] == 0xFF {
            payload_type += 255;
            pos += 1;
        }
        if pos >= rbsp.len() {
            break;
        }
        payload_type += rbsp[pos] as u32;
        pos += 1;

        // Read payload size
        let mut payload_size = 0usize;
        while pos < rbsp.len() && rbsp[pos] == 0xFF {
            payload_size += 255;
            pos += 1;
        }
        if pos >= rbsp.len() {
            break;
        }
        payload_size += rbsp[pos] as usize;
        pos += 1;

        // Extract payload
        if pos + payload_size > rbsp.len() {
            break;
        }
        let payload = &rbsp[pos..pos + payload_size];

        // Parse based on type
        let message = match payload_type {
            SEI_TYPE_MASTERING_DISPLAY_COLOUR_VOLUME => {
                parse_mastering_display_colour_volume(payload)
                    .map(SeiMessage::MasteringDisplayColourVolume)
            }
            SEI_TYPE_CONTENT_LIGHT_LEVEL_INFO => {
                parse_content_light_level_info(payload).map(SeiMessage::ContentLightLevelInfo)
            }
            SEI_TYPE_USER_DATA_REGISTERED => {
                if is_hdr10plus_payload(payload) {
                    Some(SeiMessage::Hdr10Plus)
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(msg) = message {
            messages.push(msg);
        }

        pos += payload_size;
    }

    messages
}

/// Parse Mastering Display Colour Volume SEI message
///
/// Structure (24 bytes total):
/// - display_primaries[3][2]: 6 x u16 (chromaticity coords for RGB)
/// - white_point[2]: 2 x u16 (chromaticity coords)
/// - max_display_mastering_luminance: u32
/// - min_display_mastering_luminance: u32
fn parse_mastering_display_colour_volume(payload: &[u8]) -> Option<MasteringDisplay> {
    if payload.len() < 24 {
        return None;
    }

    // Parse chromaticity values
    // Note: The order in HEVC SEI is Green, Blue, Red
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

/// Parse Content Light Level Info SEI message
///
/// Structure (4 bytes):
/// - max_content_light_level: u16
/// - max_pic_average_light_level: u16
fn parse_content_light_level_info(payload: &[u8]) -> Option<ContentLightLevel> {
    if payload.len() < 4 {
        return None;
    }

    let max_cll = u16::from_be_bytes([payload[0], payload[1]]);
    let max_fall = u16::from_be_bytes([payload[2], payload[3]]);

    Some(ContentLightLevel { max_cll, max_fall })
}

/// Check if user_data_registered payload is HDR10+ metadata
///
/// HDR10+ uses ITU-T T.35 with:
/// - country_code: 0xB5 (USA)
/// - terminal_provider_code: 0x003C (Samsung)
/// - terminal_provider_oriented_code: 0x0001 (HDR10+)
fn is_hdr10plus_payload(payload: &[u8]) -> bool {
    if payload.len() < 7 {
        return false;
    }

    // Check ITU-T T.35 terminal provider
    payload[0] == 0xB5  // country_code: USA
        && payload[1] == 0x00
        && payload[2] == 0x3C // terminal_provider_code: Samsung
        && payload[3] == 0x00
        && payload[4] == 0x01 // terminal_provider_oriented_code: HDR10+
}

/// Additional HDR10+ detection for ST 2094-40
///
/// Some HDR10+ content uses the registered user data format
/// with SMPTE ST 2094-40 application identifier
#[allow(dead_code)]
pub fn is_hdr10plus_st2094_40(payload: &[u8]) -> bool {
    if payload.len() < 5 {
        return false;
    }

    // SMPTE ST 2094-40 HDR10+ indicator
    // ITU-T T.35 country code USA + provider code for ST 2094
    payload[0] == 0xB5 && payload[1] == 0x00 && payload[2] == 0x3C
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mastering_display() {
        // Example mastering display data (24 bytes)
        // BT.2020 primaries, D65 white point, 1000 nits max, 0.0001 nits min
        let payload = [
            // Green: (0.170, 0.797) -> (8500, 39850)
            0x21, 0x34, 0x9B, 0x9A, // Red: (0.708, 0.292) -> (35400, 14600)
            0x8A, 0x48, 0x39, 0x08, // Blue: (0.131, 0.046) -> (6550, 2300)
            0x19, 0x96, 0x08, 0xFC, // White: (0.3127, 0.3290) -> (15635, 16450)
            0x3D, 0x13, 0x40, 0x42, // Max luminance: 10000000 (1000 nits)
            0x00, 0x98, 0x96, 0x80, // Min luminance: 1 (0.0001 nits)
            0x00, 0x00, 0x00, 0x01,
        ];

        let md = parse_mastering_display_colour_volume(&payload).unwrap();

        // Check that we got reasonable values
        assert!(md.max_luminance > md.min_luminance);
        assert!(md.primaries[0][0] > 0 && md.primaries[0][1] > 0);
    }

    #[test]
    fn test_parse_content_light_level() {
        // MaxCLL: 1000, MaxFALL: 400
        let payload = [0x03, 0xE8, 0x01, 0x90];

        let cll = parse_content_light_level_info(&payload).unwrap();

        assert_eq!(cll.max_cll, 1000);
        assert_eq!(cll.max_fall, 400);
    }

    #[test]
    fn test_is_hdr10plus_payload() {
        // Valid HDR10+ payload start
        let valid = [0xB5, 0x00, 0x3C, 0x00, 0x01, 0x04, 0x00];
        assert!(is_hdr10plus_payload(&valid));

        // Invalid payload
        let invalid = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert!(!is_hdr10plus_payload(&invalid));
    }
}
