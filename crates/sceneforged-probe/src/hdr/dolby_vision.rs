//! Dolby Vision detection and RPU parsing

use dolby_vision::rpu::dovi_rpu::DoviRpu;

use crate::types::HdrFormat;

/// Detect Dolby Vision from codec private data or NAL units
///
/// This function looks for Dolby Vision configuration or RPU data
pub fn detect_dolby_vision(codec_private: &[u8]) -> Option<HdrFormat> {
    // Check for Dolby Vision configuration record (dvcC/dvvC box format)
    if let Some(dv) = parse_dv_config(codec_private) {
        return Some(dv);
    }

    // Try to parse as RPU data
    parse_rpu(codec_private)
}

/// Parse Dolby Vision configuration record
///
/// Format of dvcC/dvvC box (from Dolby spec):
/// - dv_version_major (8 bits)
/// - dv_version_minor (8 bits)
/// - dv_profile (7 bits)
/// - dv_level (6 bits)
/// - rpu_present_flag (1 bit)
/// - el_present_flag (1 bit)
/// - bl_present_flag (1 bit)
/// - dv_bl_signal_compatibility_id (4 bits)
fn parse_dv_config(data: &[u8]) -> Option<HdrFormat> {
    if data.len() < 4 {
        return None;
    }

    // Check for dvcC/dvvC signature or raw config data
    let config_data = if data.len() >= 8 && (&data[4..8] == b"dvcC" || &data[4..8] == b"dvvC") {
        // Box format - skip size (4) and type (4)
        &data[8..]
    } else {
        // Raw config data
        data
    };

    if config_data.len() < 4 {
        return None;
    }

    let _version_major = config_data[0];
    let _version_minor = config_data[1];

    // Profile is in bits 1-7 of byte 2
    let profile = (config_data[2] >> 1) & 0x7F;

    // Level is in bit 0 of byte 2 and bits 7-3 of byte 3
    let level = ((config_data[2] & 0x01) << 5) | ((config_data[3] >> 3) & 0x1F);

    // Flags are in byte 3
    let rpu_present = (config_data[3] & 0x04) != 0;
    let el_present = (config_data[3] & 0x02) != 0;
    let _bl_present = (config_data[3] & 0x01) != 0;

    // BL signal compatibility ID is in byte 4
    let bl_compat_id = if config_data.len() > 4 {
        Some((config_data[4] >> 4) & 0x0F)
    } else {
        None
    };

    // Determine base layer compatibility
    let bl_signal_compatibility = bl_compat_id.map(|id| {
        Box::new(match id {
            1 => HdrFormat::Sdr,
            2 => HdrFormat::Sdr, // SDR
            4 => HdrFormat::Hdr10 {
                mastering_display: None,
                content_light_level: None,
            },
            _ => HdrFormat::Sdr,
        })
    });

    // Validate profile range
    if profile > 10 {
        return None;
    }

    Some(HdrFormat::DolbyVision {
        profile,
        level: Some(level),
        bl_compatibility_id: bl_compat_id,
        rpu_present,
        el_present,
        bl_signal_compatibility,
    })
}

/// Parse Dolby Vision RPU NAL unit using dolby_vision crate
pub fn parse_rpu(rpu_data: &[u8]) -> Option<HdrFormat> {
    // Remove NAL header if present
    let rpu_payload = if rpu_data.len() > 2 {
        // Check if this looks like a NAL unit with header
        let nal_type = (rpu_data[0] >> 1) & 0x3F;
        if nal_type == 62 {
            // UNSPEC62 - DV RPU
            &rpu_data[2..] // Skip 2-byte NAL header
        } else {
            rpu_data
        }
    } else {
        return None;
    };

    // Try to parse with dolby_vision crate
    match DoviRpu::parse_rpu(rpu_payload) {
        Ok(rpu) => {
            let header = &rpu.dovi_profile;

            // Get profile from RPU
            let profile = *header;

            // Determine compatibility based on profile
            let (bl_compat_id, bl_signal_compatibility) = match profile {
                5 => (Some(0), Some(Box::new(HdrFormat::Sdr) as Box<HdrFormat>)),
                7 => {
                    // Profile 7 can have various BL compatibility
                    // Check VDR info for more details
                    (
                        Some(4),
                        Some(Box::new(HdrFormat::Hdr10 {
                            mastering_display: None,
                            content_light_level: None,
                        })),
                    )
                }
                8 => {
                    // Profile 8 - single layer with various compatibility
                    (
                        Some(4),
                        Some(Box::new(HdrFormat::Hdr10 {
                            mastering_display: None,
                            content_light_level: None,
                        })),
                    )
                }
                _ => (None, None),
            };

            Some(HdrFormat::DolbyVision {
                profile,
                level: None, // Level not in RPU
                bl_compatibility_id: bl_compat_id,
                rpu_present: true,
                el_present: rpu.el_type.is_some(),
                bl_signal_compatibility,
            })
        }
        Err(_) => None,
    }
}

/// Check if NAL unit is a Dolby Vision RPU
pub fn is_dv_rpu_nal(nal_type: u8) -> bool {
    // HEVC UNSPEC62 is used for DV RPU
    nal_type == 62
}

/// Extract Dolby Vision profile from configuration
pub fn get_dv_profile_name(profile: u8) -> &'static str {
    match profile {
        0 => "Profile 0 (Dual-layer, BL: AVC, EL: AVC)",
        1 => "Profile 1 (Dual-layer, BL: AVC, EL: AVC)",
        2 => "Profile 2 (Dual-layer, BL: AVC, EL: AVC)",
        3 => "Profile 3 (Dual-layer, BL: AVC, EL: AVC)",
        4 => "Profile 4 (Dual-layer, BL: HEVC, EL: HEVC)",
        5 => "Profile 5 (Single-layer, HEVC, SDR compatible)",
        6 => "Profile 6 (Dual-layer, BL: HEVC, EL: HEVC)",
        7 => "Profile 7 (Dual-layer, BL: HEVC, EL: HEVC, HDR10 compatible)",
        8 => "Profile 8 (Single-layer, HEVC, HDR10/HLG compatible)",
        9 => "Profile 9 (Single-layer, AV1, HDR10 compatible)",
        10 => "Profile 10 (Single-layer, AV1)",
        _ => "Unknown Profile",
    }
}
