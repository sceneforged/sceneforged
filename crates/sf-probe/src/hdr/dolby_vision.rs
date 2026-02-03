//! Dolby Vision detection from codec private data and RPU NAL units.

use dolby_vision::rpu::dovi_rpu::DoviRpu;

use crate::types::DvInfo;

/// Attempt to detect Dolby Vision from codec private data.
///
/// Checks for a dvcC/dvvC configuration record first, then falls back
/// to RPU parsing.
pub(crate) fn detect_dolby_vision(codec_private: &[u8]) -> Option<DvInfo> {
    parse_dv_config(codec_private).or_else(|| parse_rpu_nal(codec_private))
}

/// Parse a Dolby Vision configuration record (dvcC/dvvC box format).
///
/// Layout:
/// - dv_version_major (8 bits)
/// - dv_version_minor (8 bits)
/// - dv_profile (7 bits)
/// - dv_level (6 bits)
/// - rpu_present_flag (1 bit)
/// - el_present_flag (1 bit)
/// - bl_present_flag (1 bit)
/// - dv_bl_signal_compatibility_id (4 bits)
fn parse_dv_config(data: &[u8]) -> Option<DvInfo> {
    if data.len() < 4 {
        return None;
    }

    // Skip box header if present.
    let config = if data.len() >= 8 && (&data[4..8] == b"dvcC" || &data[4..8] == b"dvvC") {
        &data[8..]
    } else {
        data
    };

    if config.len() < 4 {
        return None;
    }

    let profile = (config[2] >> 1) & 0x7F;

    let rpu_present = (config[3] & 0x04) != 0;
    let el_present = (config[3] & 0x02) != 0;
    let bl_present = (config[3] & 0x01) != 0;

    // Validate profile range.
    if profile > 10 {
        return None;
    }

    Some(DvInfo {
        profile,
        rpu_present,
        el_present,
        bl_present,
    })
}

/// Parse a Dolby Vision RPU from an HEVC NAL unit (UNSPEC62).
pub(crate) fn parse_rpu_nal(rpu_data: &[u8]) -> Option<DvInfo> {
    if rpu_data.len() <= 2 {
        return None;
    }

    // Strip NAL header if the NAL type indicates UNSPEC62.
    let payload = {
        let nal_type = (rpu_data[0] >> 1) & 0x3F;
        if nal_type == 62 {
            &rpu_data[2..]
        } else {
            rpu_data
        }
    };

    let rpu = DoviRpu::parse_rpu(payload).ok()?;

    Some(DvInfo {
        profile: rpu.dovi_profile,
        rpu_present: true,
        el_present: rpu.el_type.is_some(),
        bl_present: true,
    })
}
