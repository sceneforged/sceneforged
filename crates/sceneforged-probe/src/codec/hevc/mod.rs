//! HEVC (H.265) codec parsing
//!
//! This module provides parsing of HEVC NAL units to extract:
//! - Video Parameter Set (VPS)
//! - Sequence Parameter Set (SPS)
//! - Picture Parameter Set (PPS)
//! - Supplemental Enhancement Information (SEI)
//!
//! The primary use is extracting HDR metadata and video characteristics.

mod nal;
mod sei;
mod sps;

pub use nal::{extract_nal_units, NalUnit, NalUnitType};
pub use sei::{parse_sei, SeiMessage};
pub use sps::{parse_sps, Sps, Vui};

use crate::types::{
    ColorPrimaries, ContentLightLevel, HdrFormat, MasteringDisplay, MatrixCoefficients,
    TransferCharacteristics,
};

/// HEVC stream information extracted from bitstream
#[derive(Debug, Clone)]
pub struct HevcInfo {
    /// Sequence Parameter Set (if found)
    pub sps: Option<Sps>,
    /// Video characteristics from VUI
    pub color_primaries: Option<ColorPrimaries>,
    pub transfer_characteristics: Option<TransferCharacteristics>,
    pub matrix_coefficients: Option<MatrixCoefficients>,
    /// HDR metadata from SEI
    pub mastering_display: Option<MasteringDisplay>,
    pub content_light_level: Option<ContentLightLevel>,
    /// Detected HDR format
    pub hdr_format: Option<HdrFormat>,
    /// Dolby Vision RPU present
    pub has_dolby_vision: bool,
    /// HDR10+ metadata present
    pub has_hdr10plus: bool,
}

impl Default for HevcInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl HevcInfo {
    pub fn new() -> Self {
        Self {
            sps: None,
            color_primaries: None,
            transfer_characteristics: None,
            matrix_coefficients: None,
            mastering_display: None,
            content_light_level: None,
            hdr_format: None,
            has_dolby_vision: false,
            has_hdr10plus: false,
        }
    }
}

/// Parse HEVC codec private data (HEVCDecoderConfigurationRecord)
///
/// This is the format used in MP4 hvcC box and MKV CodecPrivate
pub fn parse_hevc_config(data: &[u8]) -> Option<HevcInfo> {
    if data.len() < 23 {
        return None;
    }

    // HEVCDecoderConfigurationRecord structure:
    // configurationVersion (8 bits)
    // general_profile_space (2 bits) + general_tier_flag (1 bit) + general_profile_idc (5 bits)
    // general_profile_compatibility_flags (32 bits)
    // general_constraint_indicator_flags (48 bits)
    // general_level_idc (8 bits)
    // reserved (4 bits) + min_spatial_segmentation_idc (12 bits)
    // reserved (6 bits) + parallelismType (2 bits)
    // reserved (6 bits) + chromaFormat (2 bits)
    // reserved (5 bits) + bitDepthLumaMinus8 (3 bits)
    // reserved (5 bits) + bitDepthChromaMinus8 (3 bits)
    // avgFrameRate (16 bits)
    // constantFrameRate (2 bits) + numTemporalLayers (3 bits) + temporalIdNested (1 bit) + lengthSizeMinusOne (2 bits)
    // numOfArrays (8 bits)
    // followed by arrays of NAL units

    let _config_version = data[0];
    let _general_profile_idc = data[1] & 0x1F;
    let _general_level_idc = data[12];

    let bit_depth_luma = (data[19] & 0x07) + 8;
    let _bit_depth_chroma = (data[20] & 0x07) + 8;

    let _length_size_minus_one = data[21] & 0x03;
    let num_arrays = data[22];

    let mut info = HevcInfo::new();
    let mut pos = 23;

    // Parse NAL unit arrays
    for _ in 0..num_arrays {
        if pos + 3 > data.len() {
            break;
        }

        let _array_completeness = (data[pos] >> 7) & 0x01;
        let nal_unit_type = data[pos] & 0x3F;
        let num_nalus = u16::from_be_bytes([data[pos + 1], data[pos + 2]]) as usize;
        pos += 3;

        for _ in 0..num_nalus {
            if pos + 2 > data.len() {
                break;
            }

            let nalu_length = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
            pos += 2;

            if pos + nalu_length > data.len() {
                break;
            }

            let nalu_data = &data[pos..pos + nalu_length];
            pos += nalu_length;

            // Parse based on NAL unit type
            match nal_unit_type {
                // SPS
                33 => {
                    if let Some(sps) = parse_sps(nalu_data) {
                        if let Some(ref vui) = sps.vui {
                            info.color_primaries = Some(ColorPrimaries::from(vui.colour_primaries));
                            info.transfer_characteristics =
                                Some(TransferCharacteristics::from(vui.transfer_characteristics));
                            info.matrix_coefficients =
                                Some(MatrixCoefficients::from(vui.matrix_coefficients));
                        }
                        info.sps = Some(sps);
                    }
                }
                // SEI prefix
                39 => {
                    let messages = parse_sei(nalu_data);
                    for msg in messages {
                        match msg {
                            SeiMessage::MasteringDisplayColourVolume(md) => {
                                info.mastering_display = Some(md);
                            }
                            SeiMessage::ContentLightLevelInfo(cll) => {
                                info.content_light_level = Some(cll);
                            }
                            SeiMessage::Hdr10Plus => {
                                info.has_hdr10plus = true;
                            }
                            _ => {}
                        }
                    }
                }
                // Dolby Vision RPU
                62 => {
                    info.has_dolby_vision = true;
                }
                _ => {}
            }
        }
    }

    // Determine HDR format
    info.hdr_format = determine_hdr_format(&info);

    // Set bit depth from config if not from SPS
    if info.sps.is_none() {
        info.sps = Some(Sps {
            width: 0,
            height: 0,
            bit_depth_luma,
            bit_depth_chroma: bit_depth_luma,
            vui: None,
        });
    }

    Some(info)
}

/// Determine HDR format from collected information
fn determine_hdr_format(info: &HevcInfo) -> Option<HdrFormat> {
    if info.has_dolby_vision {
        return Some(HdrFormat::DolbyVision {
            profile: 0, // Would need more parsing to determine
            level: None,
            bl_compatibility_id: None,
            rpu_present: true,
            el_present: false,
            bl_signal_compatibility: None,
        });
    }

    if info.has_hdr10plus {
        return Some(HdrFormat::Hdr10Plus {
            mastering_display: info.mastering_display.clone(),
            content_light_level: info.content_light_level.clone(),
        });
    }

    // Check transfer characteristics
    if let Some(tc) = &info.transfer_characteristics {
        match tc {
            TransferCharacteristics::SmpteSt2084 => {
                return Some(HdrFormat::Hdr10 {
                    mastering_display: info.mastering_display.clone(),
                    content_light_level: info.content_light_level.clone(),
                });
            }
            TransferCharacteristics::AribStdB67 => {
                return Some(HdrFormat::Hlg);
            }
            _ => {}
        }
    }

    Some(HdrFormat::Sdr)
}
