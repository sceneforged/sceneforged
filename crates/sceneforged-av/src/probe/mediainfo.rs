//! MediaInfo-based media probing.
//!
//! MediaInfo provides better detection for HDR formats and Dolby Vision
//! compared to ffprobe.

use super::types::*;
use crate::{Error, Result};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct MediaInfoOutput {
    media: MediaInfoMedia,
}

#[derive(Debug, Deserialize)]
struct MediaInfoMedia {
    #[serde(rename = "@ref")]
    #[allow(dead_code)]
    file_ref: String,
    track: Vec<MediaInfoTrack>,
}

#[derive(Debug, Deserialize)]
struct MediaInfoTrack {
    #[serde(rename = "@type")]
    track_type: String,
    #[serde(rename = "Format")]
    format: Option<String>,
    #[serde(rename = "FileSize")]
    file_size: Option<String>,
    #[serde(rename = "Duration")]
    duration: Option<String>,
    #[serde(rename = "Width")]
    width: Option<String>,
    #[serde(rename = "Height")]
    height: Option<String>,
    #[serde(rename = "FrameRate")]
    frame_rate: Option<String>,
    #[serde(rename = "BitDepth")]
    bit_depth: Option<String>,
    #[serde(rename = "HDR_Format")]
    hdr_format: Option<String>,
    #[serde(rename = "HDR_Format_Compatibility")]
    #[allow(dead_code)]
    hdr_format_compat: Option<String>,
    #[serde(rename = "Channels")]
    channels: Option<String>,
    #[serde(rename = "SamplingRate")]
    sample_rate: Option<String>,
    #[serde(rename = "Language")]
    language: Option<String>,
    #[serde(rename = "Title")]
    title: Option<String>,
    #[serde(rename = "Default")]
    default: Option<String>,
    #[serde(rename = "Forced")]
    forced: Option<String>,
    #[serde(rename = "StreamOrder")]
    #[allow(dead_code)]
    stream_order: Option<String>,
}

/// Probe a media file using mediainfo.
pub fn probe_with_mediainfo(path: &Path) -> Result<MediaInfo> {
    let output = Command::new("mediainfo")
        .args(["--Output=JSON"])
        .arg(path)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::tool_not_found("mediainfo")
            } else {
                Error::Io(e)
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::tool_failed("mediainfo", stderr.to_string()));
    }

    let json_str = String::from_utf8(output.stdout)
        .map_err(|e| Error::parse_error("mediainfo", format!("Invalid UTF-8: {}", e)))?;

    let mi_output: MediaInfoOutput = serde_json::from_str(&json_str)?;

    parse_mediainfo_output(path, mi_output)
}

fn parse_mediainfo_output(path: &Path, output: MediaInfoOutput) -> Result<MediaInfo> {
    let mut info = MediaInfo {
        file_path: path.to_path_buf(),
        file_size: 0,
        container: String::new(),
        duration: None,
        video_tracks: Vec::new(),
        audio_tracks: Vec::new(),
        subtitle_tracks: Vec::new(),
    };

    let mut video_index = 0u32;
    let mut audio_index = 0u32;
    let mut subtitle_index = 0u32;

    for track in output.media.track {
        match track.track_type.as_str() {
            "General" => {
                info.container = track.format.unwrap_or_default();
                info.file_size = track.file_size.and_then(|s| s.parse().ok()).unwrap_or(0);
                info.duration = track
                    .duration
                    .and_then(|s| s.parse::<f64>().ok())
                    .map(Duration::from_secs_f64);
            }
            "Video" => {
                let hdr_format = parse_hdr_format(track.hdr_format.as_deref());
                let dolby_vision = parse_dolby_vision(&track);

                info.video_tracks.push(VideoTrack {
                    index: video_index,
                    codec: track.format.unwrap_or_default(),
                    width: track.width.and_then(|s| parse_numeric(&s)).unwrap_or(0),
                    height: track.height.and_then(|s| parse_numeric(&s)).unwrap_or(0),
                    frame_rate: track.frame_rate.and_then(|s| s.parse().ok()),
                    bit_depth: track.bit_depth.and_then(|s| s.parse().ok()),
                    hdr_format,
                    dolby_vision,
                });
                video_index += 1;
            }
            "Audio" => {
                let codec = track.format.clone().unwrap_or_default();
                let atmos = codec.to_lowercase().contains("atmos")
                    || track
                        .title
                        .as_ref()
                        .is_some_and(|t| t.to_lowercase().contains("atmos"));

                info.audio_tracks.push(AudioTrack {
                    index: audio_index,
                    codec,
                    channels: track.channels.and_then(|s| parse_numeric(&s)).unwrap_or(2),
                    sample_rate: track.sample_rate.and_then(|s| parse_numeric(&s)),
                    language: track.language,
                    title: track.title,
                    default: track.default.as_deref() == Some("Yes"),
                    atmos,
                });
                audio_index += 1;
            }
            "Text" => {
                info.subtitle_tracks.push(SubtitleTrack {
                    index: subtitle_index,
                    codec: track.format.unwrap_or_default(),
                    language: track.language,
                    title: track.title,
                    default: track.default.as_deref() == Some("Yes"),
                    forced: track.forced.as_deref() == Some("Yes"),
                });
                subtitle_index += 1;
            }
            _ => {}
        }
    }

    Ok(info)
}

fn parse_numeric<T: std::str::FromStr>(s: &str) -> Option<T> {
    // Remove non-numeric suffixes like " pixels" or " channels"
    s.split_whitespace()
        .next()
        .and_then(|n| n.replace(' ', "").parse().ok())
}

fn parse_hdr_format(hdr_str: Option<&str>) -> Option<HdrFormat> {
    let s = hdr_str?.to_lowercase();
    if s.contains("dolby vision") {
        Some(HdrFormat::DolbyVision)
    } else if s.contains("hdr10+") || s.contains("hdr10 plus") {
        Some(HdrFormat::Hdr10Plus)
    } else if s.contains("hdr10") || s.contains("smpte st 2086") {
        Some(HdrFormat::Hdr10)
    } else if s.contains("hlg") {
        Some(HdrFormat::Hlg)
    } else {
        None
    }
}

fn parse_dolby_vision(track: &MediaInfoTrack) -> Option<DolbyVisionInfo> {
    let hdr_str = track.hdr_format.as_ref()?;
    if !hdr_str.to_lowercase().contains("dolby vision") {
        return None;
    }

    // Parse profile from HDR format string like "Dolby Vision, Version 1.0, dvhe.07.06, BL+EL+RPU"
    let profile = hdr_str
        .split(',')
        .find(|s| s.contains("dvhe.") || s.contains("dvav."))
        .and_then(|s| {
            s.trim()
                .split('.')
                .nth(1)
                .and_then(|p| p.parse::<u8>().ok())
        })
        .unwrap_or(0);

    let components = hdr_str.to_lowercase();

    Some(DolbyVisionInfo {
        profile,
        level: None,
        rpu_present: components.contains("rpu"),
        el_present: components.contains("el"),
        bl_present: components.contains("bl"),
        bl_compatibility_id: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_numeric() {
        assert_eq!(parse_numeric::<u32>("1920 pixels"), Some(1920));
        assert_eq!(parse_numeric::<u32>("1080"), Some(1080));
        assert_eq!(parse_numeric::<u32>("6 channels"), Some(6));
    }

    #[test]
    fn test_parse_hdr_format() {
        assert_eq!(
            parse_hdr_format(Some("Dolby Vision, Version 1.0")),
            Some(HdrFormat::DolbyVision)
        );
        assert_eq!(
            parse_hdr_format(Some("SMPTE ST 2086")),
            Some(HdrFormat::Hdr10)
        );
        assert_eq!(parse_hdr_format(Some("HDR10+")), Some(HdrFormat::Hdr10Plus));
        assert_eq!(parse_hdr_format(None), None);
    }
}
