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
    // Check color transfer characteristic for HDR
    let transfer = video.color_transfer_characteristic();
    let primaries = video.color_primaries();

    // Check for Dolby Vision in side data
    // FFmpeg exposes DV config through side data, but it's complex to parse
    // For now, we detect based on color characteristics

    let transfer_str = format!("{:?}", transfer).to_lowercase();
    let primaries_str = format!("{:?}", primaries).to_lowercase();

    // Check stream metadata for Dolby Vision hints
    let has_dovi_hint = stream.metadata().iter().any(|(k, v)| {
        let k_lower = k.to_lowercase();
        let v_lower = v.to_lowercase();
        k_lower.contains("dovi") || v_lower.contains("dolby vision")
    });

    if has_dovi_hint {
        return (
            Some(HdrFormat::DolbyVision),
            Some(DolbyVisionInfo {
                profile: 0, // Would need to parse side data for actual profile
                level: None,
                rpu_present: true,
                el_present: false,
                bl_present: true,
                bl_compatibility_id: None,
            }),
        );
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_ffmpeg() {
        // Just verify initialization doesn't panic
        init_ffmpeg();
        init_ffmpeg(); // Should be idempotent
    }
}
