//! Native FFmpeg-based media probing using ffmpeg-the-third bindings.
//!
//! This module provides direct access to FFmpeg libraries without subprocess
//! spawning or JSON parsing. Requires the `native-ffmpeg` feature.

use super::types::*;
use crate::{Error, Result};
use ffmpeg_the_third as ffmpeg;
use std::path::Path;
use std::sync::Once;
use std::time::Duration;

static FFMPEG_INIT: Once = Once::new();

fn init_ffmpeg() {
    FFMPEG_INIT.call_once(|| {
        ffmpeg::init().expect("Failed to initialize FFmpeg");
    });
}

/// Probe a media file using native FFmpeg bindings.
///
/// This provides direct struct access without JSON parsing overhead.
pub fn probe_with_native_ffmpeg(path: &Path) -> Result<MediaInfo> {
    init_ffmpeg();

    let context = ffmpeg::format::input(path).map_err(|e| {
        if e.to_string().contains("No such file") {
            Error::file_not_found(path)
        } else {
            Error::tool_failed("ffmpeg", e.to_string())
        }
    })?;

    let duration = if context.duration() > 0 {
        Some(Duration::from_micros(context.duration() as u64))
    } else {
        None
    };

    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    // Detect container format
    let container = context
        .format()
        .name()
        .split(',')
        .next()
        .unwrap_or("unknown")
        .to_string();

    let mut video_tracks = Vec::new();
    let mut audio_tracks = Vec::new();
    let mut subtitle_tracks = Vec::new();

    let mut video_index = 0u32;
    let mut audio_index = 0u32;
    let mut subtitle_index = 0u32;

    for stream in context.streams() {
        let params = stream.parameters();
        let codec_ctx = ffmpeg::codec::context::Context::from_parameters(params).map_err(|e| {
            Error::parse_error("ffmpeg", format!("Failed to get codec context: {}", e))
        })?;

        match codec_ctx.medium() {
            ffmpeg::media::Type::Video => {
                let codec_id = codec_ctx.id();
                if let Ok(video) = codec_ctx.decoder().video() {
                    let (hdr_format, dolby_vision) = detect_hdr_info(&video, &stream);

                    video_tracks.push(VideoTrack {
                        index: video_index,
                        codec: format!("{:?}", codec_id),
                        width: video.width(),
                        height: video.height(),
                        frame_rate: parse_frame_rate(stream.rate()),
                        bit_depth: detect_bit_depth(&video),
                        hdr_format,
                        dolby_vision,
                    });
                    video_index += 1;
                }
            }
            ffmpeg::media::Type::Audio => {
                let codec_id = codec_ctx.id();
                if let Ok(audio) = codec_ctx.decoder().audio() {
                    let language = stream.metadata().get("language").map(|s| s.to_string());
                    let title = stream.metadata().get("title").map(|s| s.to_string());

                    let channels = audio.ch_layout().channels();

                    // Detect Atmos from title or codec
                    let codec_name = format!("{:?}", codec_id);
                    let atmos = title
                        .as_ref()
                        .map(|t| t.to_lowercase().contains("atmos"))
                        .unwrap_or(false)
                        || codec_name.to_lowercase().contains("truehd");

                    audio_tracks.push(AudioTrack {
                        index: audio_index,
                        codec: codec_name,
                        channels,
                        sample_rate: Some(audio.rate()),
                        language,
                        title,
                        default: stream
                            .disposition()
                            .contains(ffmpeg::format::stream::Disposition::DEFAULT),
                        atmos,
                    });
                    audio_index += 1;
                }
            }
            ffmpeg::media::Type::Subtitle => {
                let language = stream.metadata().get("language").map(|s| s.to_string());
                let title = stream.metadata().get("title").map(|s| s.to_string());

                subtitle_tracks.push(SubtitleTrack {
                    index: subtitle_index,
                    codec: format!("{:?}", codec_ctx.id()),
                    language,
                    title,
                    default: stream
                        .disposition()
                        .contains(ffmpeg::format::stream::Disposition::DEFAULT),
                    forced: stream
                        .disposition()
                        .contains(ffmpeg::format::stream::Disposition::FORCED),
                });
                subtitle_index += 1;
            }
            _ => {}
        }
    }

    Ok(MediaInfo {
        file_path: path.to_path_buf(),
        file_size,
        container,
        duration,
        video_tracks,
        audio_tracks,
        subtitle_tracks,
    })
}

fn parse_frame_rate(rate: ffmpeg::Rational) -> Option<f64> {
    if rate.denominator() != 0 {
        Some(rate.numerator() as f64 / rate.denominator() as f64)
    } else {
        None
    }
}

fn detect_bit_depth(video: &ffmpeg::decoder::Video) -> Option<u8> {
    // Detect bit depth from pixel format
    let format = video.format();
    let format_str = format!("{:?}", format).to_lowercase();

    if format_str.contains("10le") || format_str.contains("10be") || format_str.contains("p010") {
        Some(10)
    } else if format_str.contains("12le") || format_str.contains("12be") {
        Some(12)
    } else if format_str.contains("yuv420p")
        || format_str.contains("yuv422p")
        || format_str.contains("yuv444p")
    {
        Some(8)
    } else {
        None
    }
}

fn detect_hdr_info(
    video: &ffmpeg::decoder::Video,
    stream: &ffmpeg::format::stream::Stream,
) -> (Option<HdrFormat>, Option<DolbyVisionInfo>) {
    // 1. Check coded_side_data on codec parameters for DOVI configuration record.
    //    In FFmpeg 7.0+, stream-level side_data moved to AVCodecParameters::coded_side_data.
    if let Some(dv) = read_dovi_from_coded_side_data(stream) {
        return (Some(HdrFormat::DolbyVision), Some(dv));
    }

    // 2. Fall back to color transfer characteristics for HDR10/HLG
    let transfer = video.color_transfer_characteristic();
    let primaries = video.color_primaries();

    let transfer_str = format!("{:?}", transfer).to_lowercase();
    let primaries_str = format!("{:?}", primaries).to_lowercase();

    // PQ transfer = HDR10 or HDR10+
    if transfer_str.contains("smpte2084") || transfer_str.contains("pq") {
        // BT.2020 primaries typically indicate HDR10
        if primaries_str.contains("bt2020") {
            return (Some(HdrFormat::Hdr10), None);
        }
    }

    // HLG transfer
    if transfer_str.contains("arib") || transfer_str.contains("hlg") {
        return (Some(HdrFormat::Hlg), None);
    }

    (None, None)
}

/// Read Dolby Vision configuration from AVCodecParameters::coded_side_data.
///
/// The safe `ffmpeg-the-third` wrapper doesn't expose `coded_side_data` yet (marked TODO),
/// so we access the raw `AVCodecParameters` struct fields via unsafe FFI.
fn read_dovi_from_coded_side_data(
    stream: &ffmpeg::format::stream::Stream,
) -> Option<DolbyVisionInfo> {
    let params_ptr = stream.parameters().as_ptr();

    // SAFETY: params_ptr is guaranteed non-null by ParametersRef. We read two fields
    // (coded_side_data pointer and nb_coded_side_data count) from the valid AVCodecParameters.
    let (side_data_ptr, count) = unsafe {
        let nb = (*params_ptr).nb_coded_side_data;
        let ptr = (*params_ptr).coded_side_data;
        if nb <= 0 || ptr.is_null() {
            return None;
        }
        (ptr, nb as usize)
    };

    let dovi_conf_type = ffmpeg::ffi::AVPacketSideDataType::AV_PKT_DATA_DOVI_CONF;

    for i in 0..count {
        // SAFETY: We're iterating within the bounds [0, nb_coded_side_data).
        // Each element is a valid AVPacketSideData struct.
        let sd = unsafe { &*side_data_ptr.add(i) };
        if sd.type_ == dovi_conf_type && !sd.data.is_null() && sd.size >= 4 {
            // SAFETY: data is non-null with at least `sd.size` bytes.
            let data = unsafe { std::slice::from_raw_parts(sd.data, sd.size) };
            if let Some((profile, level, rpu, el, bl, compat_id)) = parse_dovi_config(data) {
                return Some(DolbyVisionInfo {
                    profile,
                    level: Some(level),
                    rpu_present: rpu,
                    el_present: el,
                    bl_present: bl,
                    bl_compatibility_id: Some(compat_id),
                });
            }
        }
    }

    None
}

/// Parse a DOVI configuration record (dvcC/dvvC box format).
///
/// Layout (first 5 bytes):
/// - byte 0: dv_version_major
/// - byte 1: dv_version_minor
/// - byte 2: (profile << 1) | (level >> 5)
/// - byte 3: (level << 3) | (rpu_flag << 2) | (el_flag << 1) | bl_flag
/// - byte 4: (bl_compatibility_id << 4) | ...
fn parse_dovi_config(data: &[u8]) -> Option<(u8, u8, bool, bool, bool, u8)> {
    if data.len() < 4 {
        return None;
    }
    let profile = (data[2] >> 1) & 0x7F;
    let level = ((data[2] & 0x01) << 5) | ((data[3] >> 3) & 0x1F);
    let rpu = (data[3] & 0x04) != 0;
    let el = (data[3] & 0x02) != 0;
    let bl = (data[3] & 0x01) != 0;
    let compat_id = if data.len() > 4 {
        (data[4] >> 4) & 0x0F
    } else {
        0
    };
    if profile > 10 {
        return None;
    }
    Some((profile, level, rpu, el, bl, compat_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_ffmpeg() {
        // Just verify initialization doesn't panic
        init_ffmpeg();
        init_ffmpeg(); // Should be idempotent
    }

    #[test]
    fn test_parse_dovi_config_profile7() {
        // Profile 7, level 6, RPU=true, EL=true, BL=true, compat_id=6
        // byte 2: (7 << 1) | (6 >> 5) = 14 | 0 = 0x0E
        // byte 3: (6 << 3) | (1 << 2) | (1 << 1) | 1 = 48 | 4 | 2 | 1 = 0x37
        // byte 4: (6 << 4) = 0x60
        let data = [1, 0, 0x0E, 0x37, 0x60];
        let result = parse_dovi_config(&data);
        assert!(result.is_some());
        let (profile, level, rpu, el, bl, compat_id) = result.unwrap();
        assert_eq!(profile, 7);
        assert_eq!(level, 6);
        assert!(rpu);
        assert!(el);
        assert!(bl);
        assert_eq!(compat_id, 6);
    }

    #[test]
    fn test_parse_dovi_config_profile8() {
        // Profile 8, level 5, RPU=true, EL=false, BL=true, compat_id=4
        // byte 2: (8 << 1) | (5 >> 5) = 16 | 0 = 0x10
        // byte 3: (5 << 3) | (1 << 2) | (0 << 1) | 1 = 40 | 4 | 0 | 1 = 0x2D
        // byte 4: (4 << 4) = 0x40
        let data = [1, 0, 0x10, 0x2D, 0x40];
        let result = parse_dovi_config(&data);
        assert!(result.is_some());
        let (profile, level, rpu, el, bl, compat_id) = result.unwrap();
        assert_eq!(profile, 8);
        assert_eq!(level, 5);
        assert!(rpu);
        assert!(!el);
        assert!(bl);
        assert_eq!(compat_id, 4);
    }

    #[test]
    fn test_parse_dovi_config_too_short() {
        let data = [1, 0, 0x0E];
        assert!(parse_dovi_config(&data).is_none());
    }

    #[test]
    fn test_parse_dovi_config_invalid_profile() {
        // Profile 15 (invalid, > 10)
        // byte 2: (15 << 1) = 0x1E
        let data = [1, 0, 0x1E, 0x00, 0x00];
        assert!(parse_dovi_config(&data).is_none());
    }

    #[test]
    fn test_parse_dovi_config_no_compat_byte() {
        // Only 4 bytes, compat_id should default to 0
        // Profile 5, level 0
        // byte 2: (5 << 1) = 0x0A
        // byte 3: (0 << 3) | (1 << 2) | (0 << 1) | 1 = 0x05
        let data = [1, 0, 0x0A, 0x05];
        let result = parse_dovi_config(&data);
        assert!(result.is_some());
        let (profile, _level, rpu, el, bl, compat_id) = result.unwrap();
        assert_eq!(profile, 5);
        assert!(rpu);
        assert!(!el);
        assert!(bl);
        assert_eq!(compat_id, 0);
    }
}
