//! HEVC Sequence Parameter Set (SPS) parsing

use super::nal::remove_emulation_prevention;

/// Sequence Parameter Set
#[derive(Debug, Clone)]
pub struct Sps {
    /// Picture width in luma samples
    pub width: u32,
    /// Picture height in luma samples
    pub height: u32,
    /// Bit depth for luma samples
    pub bit_depth_luma: u8,
    /// Bit depth for chroma samples
    pub bit_depth_chroma: u8,
    /// Video Usability Information
    pub vui: Option<Vui>,
}

/// Video Usability Information
#[derive(Debug, Clone)]
pub struct Vui {
    /// Colour primaries (ITU-T H.273)
    pub colour_primaries: u8,
    /// Transfer characteristics (ITU-T H.273)
    pub transfer_characteristics: u8,
    /// Matrix coefficients (ITU-T H.273)
    pub matrix_coefficients: u8,
    /// Video is full range (0-255) vs limited range (16-235)
    pub video_full_range: bool,
}

/// Parse SPS NAL unit
pub fn parse_sps(data: &[u8]) -> Option<Sps> {
    if data.len() < 3 {
        return None;
    }

    // Remove emulation prevention bytes for proper parsing
    let rbsp = remove_emulation_prevention(data);

    // Skip NAL header (2 bytes)
    let mut reader = BitReader::new(&rbsp[2..]);

    // sps_video_parameter_set_id (4 bits)
    reader.read_bits(4)?;

    // sps_max_sub_layers_minus1 (3 bits)
    let max_sub_layers_minus1 = reader.read_bits(3)? as u8;

    // sps_temporal_id_nesting_flag (1 bit)
    reader.read_bits(1)?;

    // profile_tier_level
    parse_profile_tier_level(&mut reader, max_sub_layers_minus1)?;

    // sps_seq_parameter_set_id (ue(v))
    reader.read_ue()?;

    // chroma_format_idc (ue(v))
    let chroma_format_idc = reader.read_ue()?;

    if chroma_format_idc == 3 {
        // separate_colour_plane_flag (1 bit)
        reader.read_bits(1)?;
    }

    // pic_width_in_luma_samples (ue(v))
    let width = reader.read_ue()?;

    // pic_height_in_luma_samples (ue(v))
    let height = reader.read_ue()?;

    // conformance_window_flag (1 bit)
    let conformance_window_flag = reader.read_bits(1)?;
    if conformance_window_flag == 1 {
        // conf_win_left_offset (ue(v))
        reader.read_ue()?;
        // conf_win_right_offset (ue(v))
        reader.read_ue()?;
        // conf_win_top_offset (ue(v))
        reader.read_ue()?;
        // conf_win_bottom_offset (ue(v))
        reader.read_ue()?;
    }

    // bit_depth_luma_minus8 (ue(v))
    let bit_depth_luma = (reader.read_ue()? + 8) as u8;

    // bit_depth_chroma_minus8 (ue(v))
    let bit_depth_chroma = (reader.read_ue()? + 8) as u8;

    // log2_max_pic_order_cnt_lsb_minus4 (ue(v))
    reader.read_ue()?;

    // sps_sub_layer_ordering_info_present_flag (1 bit)
    let sub_layer_ordering_info_present = reader.read_bits(1)?;

    let start = if sub_layer_ordering_info_present == 1 {
        0
    } else {
        max_sub_layers_minus1
    };

    for _ in start..=max_sub_layers_minus1 {
        reader.read_ue()?; // sps_max_dec_pic_buffering_minus1
        reader.read_ue()?; // sps_max_num_reorder_pics
        reader.read_ue()?; // sps_max_latency_increase_plus1
    }

    // log2_min_luma_coding_block_size_minus3 (ue(v))
    reader.read_ue()?;
    // log2_diff_max_min_luma_coding_block_size (ue(v))
    reader.read_ue()?;
    // log2_min_luma_transform_block_size_minus2 (ue(v))
    reader.read_ue()?;
    // log2_diff_max_min_luma_transform_block_size (ue(v))
    reader.read_ue()?;
    // max_transform_hierarchy_depth_inter (ue(v))
    reader.read_ue()?;
    // max_transform_hierarchy_depth_intra (ue(v))
    reader.read_ue()?;

    // scaling_list_enabled_flag (1 bit)
    let scaling_list_enabled = reader.read_bits(1)?;
    if scaling_list_enabled == 1 {
        // sps_scaling_list_data_present_flag (1 bit)
        let scaling_list_data_present = reader.read_bits(1)?;
        if scaling_list_data_present == 1 {
            // Skip scaling list data (complex, not needed for HDR detection)
            // This is a simplified parser
        }
    }

    // amp_enabled_flag (1 bit)
    reader.read_bits(1)?;
    // sample_adaptive_offset_enabled_flag (1 bit)
    reader.read_bits(1)?;

    // pcm_enabled_flag (1 bit)
    let pcm_enabled = reader.read_bits(1)?;
    if pcm_enabled == 1 {
        reader.read_bits(4)?; // pcm_sample_bit_depth_luma_minus1
        reader.read_bits(4)?; // pcm_sample_bit_depth_chroma_minus1
        reader.read_ue()?; // log2_min_pcm_luma_coding_block_size_minus3
        reader.read_ue()?; // log2_diff_max_min_pcm_luma_coding_block_size
        reader.read_bits(1)?; // pcm_loop_filter_disabled_flag
    }

    // num_short_term_ref_pic_sets (ue(v))
    let num_short_term_ref_pic_sets = reader.read_ue()?;

    // Skip short-term ref pic sets (complex)
    for i in 0..num_short_term_ref_pic_sets {
        skip_short_term_ref_pic_set(&mut reader, i, num_short_term_ref_pic_sets)?;
    }

    // long_term_ref_pics_present_flag (1 bit)
    let long_term_ref_pics_present = reader.read_bits(1)?;
    if long_term_ref_pics_present == 1 {
        let num_long_term_ref_pics = reader.read_ue()?;
        for _ in 0..num_long_term_ref_pics {
            reader.read_ue()?; // lt_ref_pic_poc_lsb_sps
            reader.read_bits(1)?; // used_by_curr_pic_lt_sps_flag
        }
    }

    // sps_temporal_mvp_enabled_flag (1 bit)
    reader.read_bits(1)?;
    // strong_intra_smoothing_enabled_flag (1 bit)
    reader.read_bits(1)?;

    // vui_parameters_present_flag (1 bit)
    let vui_present = reader.read_bits(1)?;
    let vui = if vui_present == 1 {
        parse_vui(&mut reader, max_sub_layers_minus1)
    } else {
        None
    };

    Some(Sps {
        width,
        height,
        bit_depth_luma,
        bit_depth_chroma,
        vui,
    })
}

/// Parse profile_tier_level structure
fn parse_profile_tier_level(reader: &mut BitReader, max_sub_layers_minus1: u8) -> Option<()> {
    // general_profile_space (2 bits)
    reader.read_bits(2)?;
    // general_tier_flag (1 bit)
    reader.read_bits(1)?;
    // general_profile_idc (5 bits)
    reader.read_bits(5)?;
    // general_profile_compatibility_flag[32] (32 bits)
    reader.read_bits(32)?;
    // general_progressive_source_flag (1 bit)
    reader.read_bits(1)?;
    // general_interlaced_source_flag (1 bit)
    reader.read_bits(1)?;
    // general_non_packed_constraint_flag (1 bit)
    reader.read_bits(1)?;
    // general_frame_only_constraint_flag (1 bit)
    reader.read_bits(1)?;
    // general_reserved_zero_44bits (44 bits)
    reader.read_bits(32)?;
    reader.read_bits(12)?;
    // general_level_idc (8 bits)
    reader.read_bits(8)?;

    let mut sub_layer_profile_present = vec![false; max_sub_layers_minus1 as usize];
    let mut sub_layer_level_present = vec![false; max_sub_layers_minus1 as usize];

    for i in 0..max_sub_layers_minus1 as usize {
        sub_layer_profile_present[i] = reader.read_bits(1)? == 1;
        sub_layer_level_present[i] = reader.read_bits(1)? == 1;
    }

    if max_sub_layers_minus1 > 0 {
        for _ in max_sub_layers_minus1..8 {
            reader.read_bits(2)?; // reserved_zero_2bits
        }
    }

    for i in 0..max_sub_layers_minus1 as usize {
        if sub_layer_profile_present[i] {
            reader.read_bits(2)?; // sub_layer_profile_space
            reader.read_bits(1)?; // sub_layer_tier_flag
            reader.read_bits(5)?; // sub_layer_profile_idc
            reader.read_bits(32)?; // sub_layer_profile_compatibility_flag
            reader.read_bits(1)?; // sub_layer_progressive_source_flag
            reader.read_bits(1)?; // sub_layer_interlaced_source_flag
            reader.read_bits(1)?; // sub_layer_non_packed_constraint_flag
            reader.read_bits(1)?; // sub_layer_frame_only_constraint_flag
            reader.read_bits(32)?;
            reader.read_bits(12)?;
        }
        if sub_layer_level_present[i] {
            reader.read_bits(8)?; // sub_layer_level_idc
        }
    }

    Some(())
}

/// Skip short-term ref pic set (simplified)
fn skip_short_term_ref_pic_set(reader: &mut BitReader, idx: u32, _num_sets: u32) -> Option<()> {
    let inter_ref_pic_set_prediction_flag = if idx > 0 { reader.read_bits(1)? } else { 0 };

    if inter_ref_pic_set_prediction_flag == 1 {
        // Complex inter prediction - skip for now
        // This is a simplified parser
        return Some(());
    }

    let num_negative_pics = reader.read_ue()?;
    let num_positive_pics = reader.read_ue()?;

    for _ in 0..num_negative_pics {
        reader.read_ue()?; // delta_poc_s0_minus1
        reader.read_bits(1)?; // used_by_curr_pic_s0_flag
    }

    for _ in 0..num_positive_pics {
        reader.read_ue()?; // delta_poc_s1_minus1
        reader.read_bits(1)?; // used_by_curr_pic_s1_flag
    }

    Some(())
}

/// Parse VUI parameters
fn parse_vui(reader: &mut BitReader, _max_sub_layers_minus1: u8) -> Option<Vui> {
    // aspect_ratio_info_present_flag (1 bit)
    let aspect_ratio_info_present = reader.read_bits(1)?;
    if aspect_ratio_info_present == 1 {
        let aspect_ratio_idc = reader.read_bits(8)?;
        if aspect_ratio_idc == 255 {
            // Extended_SAR
            reader.read_bits(16)?; // sar_width
            reader.read_bits(16)?; // sar_height
        }
    }

    // overscan_info_present_flag (1 bit)
    let overscan_info_present = reader.read_bits(1)?;
    if overscan_info_present == 1 {
        reader.read_bits(1)?; // overscan_appropriate_flag
    }

    // video_signal_type_present_flag (1 bit)
    let video_signal_type_present = reader.read_bits(1)?;

    let mut colour_primaries = 2; // Unspecified
    let mut transfer_characteristics = 2; // Unspecified
    let mut matrix_coefficients = 2; // Unspecified
    let mut video_full_range = false;

    if video_signal_type_present == 1 {
        reader.read_bits(3)?; // video_format
        video_full_range = reader.read_bits(1)? == 1;

        // colour_description_present_flag (1 bit)
        let colour_description_present = reader.read_bits(1)?;
        if colour_description_present == 1 {
            colour_primaries = reader.read_bits(8)? as u8;
            transfer_characteristics = reader.read_bits(8)? as u8;
            matrix_coefficients = reader.read_bits(8)? as u8;
        }
    }

    Some(Vui {
        colour_primaries,
        transfer_characteristics,
        matrix_coefficients,
        video_full_range,
    })
}

/// Simple bit reader for parsing RBSP data
struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    /// Read n bits (up to 32)
    fn read_bits(&mut self, n: u8) -> Option<u32> {
        let mut result = 0u32;

        for _ in 0..n {
            if self.byte_pos >= self.data.len() {
                return None;
            }

            let bit = (self.data[self.byte_pos] >> (7 - self.bit_pos)) & 1;
            result = (result << 1) | (bit as u32);

            self.bit_pos += 1;
            if self.bit_pos == 8 {
                self.bit_pos = 0;
                self.byte_pos += 1;
            }
        }

        Some(result)
    }

    /// Read unsigned Exp-Golomb coded value
    fn read_ue(&mut self) -> Option<u32> {
        // Count leading zeros
        let mut leading_zeros = 0u8;
        loop {
            let bit = self.read_bits(1)?;
            if bit == 1 {
                break;
            }
            leading_zeros += 1;
            if leading_zeros > 31 {
                return None;
            }
        }

        if leading_zeros == 0 {
            return Some(0);
        }

        let suffix = self.read_bits(leading_zeros)?;
        Some((1 << leading_zeros) - 1 + suffix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bit_reader_read_bits() {
        let data = [0b10110100, 0b01010101];
        let mut reader = BitReader::new(&data);

        assert_eq!(reader.read_bits(4), Some(0b1011));
        assert_eq!(reader.read_bits(4), Some(0b0100));
        assert_eq!(reader.read_bits(8), Some(0b01010101));
    }

    #[test]
    fn test_bit_reader_read_ue() {
        // 1 -> 0 (1 bit: 1)
        // 010 -> 1 (3 bits: 010)
        // 011 -> 2 (3 bits: 011)
        // 00100 -> 3 (5 bits: 00100)

        let data = [0b10100110, 0b01000000];
        let mut reader = BitReader::new(&data);

        assert_eq!(reader.read_ue(), Some(0)); // 1
        assert_eq!(reader.read_ue(), Some(1)); // 010
        assert_eq!(reader.read_ue(), Some(2)); // 011
        assert_eq!(reader.read_ue(), Some(3)); // 00100
    }
}
