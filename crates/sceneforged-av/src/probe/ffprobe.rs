//! FFprobe-based media probing.

use super::types::*;
use crate::{Error, Result};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    format: FfprobeFormat,
    streams: Vec<FfprobeStream>,
}

#[derive(Debug, Deserialize)]
struct FfprobeFormat {
    #[allow(dead_code)]
    filename: String,
    format_name: String,
    duration: Option<String>,
    size: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    #[allow(dead_code)]
    index: u32,
    codec_type: String,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    r_frame_rate: Option<String>,
    channels: Option<u32>,
    sample_rate: Option<String>,
    #[serde(default)]
    disposition: FfprobeDisposition,
    #[serde(default)]
    tags: FfprobeTags,
    #[serde(default)]
    side_data_list: Vec<FfprobeSideData>,
}

#[derive(Debug, Default, Deserialize)]
struct FfprobeDisposition {
    #[serde(default)]
    default: u8,
    #[serde(default)]
    forced: u8,
}

#[derive(Debug, Default, Deserialize)]
struct FfprobeTags {
    language: Option<String>,
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FfprobeSideData {
    side_data_type: Option<String>,
}

/// Probe a media file using ffprobe.
pub fn probe_with_ffprobe(path: &Path) -> Result<MediaInfo> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::tool_not_found("ffprobe")
            } else {
                Error::Io(e)
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::tool_failed("ffprobe", stderr.to_string()));
    }

    let json_str = String::from_utf8(output.stdout)
        .map_err(|e| Error::parse_error("ffprobe", format!("Invalid UTF-8: {}", e)))?;

    let ff_output: FfprobeOutput = serde_json::from_str(&json_str)?;

    parse_ffprobe_output(path, ff_output)
}

fn parse_ffprobe_output(path: &Path, output: FfprobeOutput) -> Result<MediaInfo> {
    let duration = output
        .format
        .duration
        .and_then(|s| s.parse::<f64>().ok())
        .map(Duration::from_secs_f64);

    let mut info = MediaInfo {
        file_path: path.to_path_buf(),
        file_size: output.format.size.and_then(|s| s.parse().ok()).unwrap_or(0),
        container: output.format.format_name,
        duration,
        video_tracks: Vec::new(),
        audio_tracks: Vec::new(),
        subtitle_tracks: Vec::new(),
    };

    let mut video_index = 0u32;
    let mut audio_index = 0u32;
    let mut subtitle_index = 0u32;

    for stream in output.streams {
        match stream.codec_type.as_str() {
            "video" => {
                // Check for HDR/DV via side data
                let has_dovi = stream
                    .side_data_list
                    .iter()
                    .any(|sd| sd.side_data_type.as_deref() == Some("DOVI configuration record"));

                info.video_tracks.push(VideoTrack {
                    index: video_index,
                    codec: stream.codec_name.unwrap_or_default(),
                    width: stream.width.unwrap_or(0),
                    height: stream.height.unwrap_or(0),
                    frame_rate: stream.r_frame_rate.and_then(|s| parse_frame_rate(&s)),
                    bit_depth: None,
                    hdr_format: if has_dovi {
                        Some(HdrFormat::DolbyVision)
                    } else {
                        None
                    },
                    dolby_vision: if has_dovi {
                        Some(DolbyVisionInfo {
                            profile: 0, // ffprobe doesn't easily expose this
                            level: None,
                            rpu_present: true,
                            el_present: false,
                            bl_present: true,
                            bl_compatibility_id: None,
                        })
                    } else {
                        None
                    },
                });
                video_index += 1;
            }
            "audio" => {
                info.audio_tracks.push(AudioTrack {
                    index: audio_index,
                    codec: stream.codec_name.unwrap_or_default(),
                    channels: stream.channels.unwrap_or(2),
                    sample_rate: stream.sample_rate.and_then(|s| s.parse().ok()),
                    language: stream.tags.language,
                    title: stream.tags.title,
                    default: stream.disposition.default == 1,
                    atmos: false, // Hard to detect via ffprobe
                });
                audio_index += 1;
            }
            "subtitle" => {
                info.subtitle_tracks.push(SubtitleTrack {
                    index: subtitle_index,
                    codec: stream.codec_name.unwrap_or_default(),
                    language: stream.tags.language,
                    title: stream.tags.title,
                    default: stream.disposition.default == 1,
                    forced: stream.disposition.forced == 1,
                });
                subtitle_index += 1;
            }
            _ => {}
        }
    }

    Ok(info)
}

fn parse_frame_rate(rate_str: &str) -> Option<f64> {
    let parts: Vec<&str> = rate_str.split('/').collect();
    if parts.len() == 2 {
        let num: f64 = parts[0].parse().ok()?;
        let den: f64 = parts[1].parse().ok()?;
        if den != 0.0 {
            return Some(num / den);
        }
    }
    rate_str.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frame_rate() {
        assert_eq!(parse_frame_rate("24000/1001"), Some(23.976023976023978));
        assert_eq!(parse_frame_rate("30/1"), Some(30.0));
        assert_eq!(parse_frame_rate("25"), Some(25.0));
        assert_eq!(parse_frame_rate("invalid"), None);
    }
}
