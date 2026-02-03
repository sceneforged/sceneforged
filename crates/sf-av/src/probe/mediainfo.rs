//! MediaInfo-based [`sf_probe::Prober`] implementation.
//!
//! Shells out to `mediainfo --Output=JSON <file>` and maps the JSON output
//! into [`sf_probe::MediaInfo`].  MediaInfo typically provides better HDR and
//! Dolby Vision metadata detection than ffprobe.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use sf_core::{AudioCodec, Container, HdrFormat, VideoCodec};
use sf_probe::types::{AudioTrack, DvInfo, MediaInfo, SubtitleTrack, VideoTrack};
use sf_probe::Prober;

use crate::command::ToolCommand;

/// A prober backed by the `mediainfo` CLI.
#[derive(Debug, Clone)]
pub struct MediaInfoProber {
    mediainfo_path: PathBuf,
}

impl MediaInfoProber {
    /// Create a new prober using the given mediainfo path.
    pub fn new(mediainfo_path: PathBuf) -> Self {
        Self { mediainfo_path }
    }

    /// Create a prober that finds mediainfo on `PATH`.
    pub fn from_path() -> Option<Self> {
        which::which("mediainfo")
            .ok()
            .map(|p| Self { mediainfo_path: p })
    }
}

impl Prober for MediaInfoProber {
    fn name(&self) -> &'static str {
        "mediainfo"
    }

    fn probe(&self, path: &Path) -> sf_core::Result<MediaInfo> {
        let rt = tokio::runtime::Handle::try_current();
        match rt {
            Ok(handle) => {
                tokio::task::block_in_place(|| handle.block_on(self.probe_async(path)))
            }
            Err(_) => {
                let rt = tokio::runtime::Runtime::new().map_err(|e| sf_core::Error::Tool {
                    tool: "mediainfo".into(),
                    message: format!("failed to create tokio runtime: {e}"),
                })?;
                rt.block_on(self.probe_async(path))
            }
        }
    }

    fn supports(&self, path: &Path) -> bool {
        path.extension().is_some()
    }
}

impl MediaInfoProber {
    async fn probe_async(&self, path: &Path) -> sf_core::Result<MediaInfo> {
        let mut cmd = ToolCommand::new(self.mediainfo_path.clone());
        cmd.arg("--Output=JSON");
        cmd.arg(path.to_string_lossy().as_ref());

        let output = cmd.execute().await?;
        let mi: MiOutput = serde_json::from_str(&output.stdout).map_err(|e| {
            sf_core::Error::Probe(format!("mediainfo JSON parse error: {e}"))
        })?;

        parse_mi_output(path, mi)
    }
}

// ---------------------------------------------------------------------------
// JSON structures
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct MiOutput {
    media: MiMedia,
}

#[derive(Debug, Deserialize)]
struct MiMedia {
    #[serde(default)]
    track: Vec<MiTrack>,
}

#[derive(Debug, Deserialize)]
struct MiTrack {
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
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn parse_mi_output(path: &Path, mi: MiOutput) -> sf_core::Result<MediaInfo> {
    let mut info = MediaInfo {
        file_path: path.to_path_buf(),
        file_size: 0,
        container: Container::Mkv, // will be overwritten by General track
        duration: None,
        video_tracks: Vec::new(),
        audio_tracks: Vec::new(),
        subtitle_tracks: Vec::new(),
    };

    for track in mi.media.track {
        match track.track_type.as_str() {
            "General" => {
                info.container = map_container(track.format.as_deref().unwrap_or(""));
                info.file_size = track
                    .file_size
                    .and_then(|s| parse_numeric::<u64>(&s))
                    .unwrap_or(0);
                info.duration = track
                    .duration
                    .and_then(|s| s.parse::<f64>().ok())
                    .map(Duration::from_secs_f64);
            }
            "Video" => {
                let hdr = parse_hdr_format(track.hdr_format.as_deref());
                let dv = parse_dolby_vision(&track);
                let codec = map_video_codec(track.format.as_deref().unwrap_or(""));

                info.video_tracks.push(VideoTrack {
                    codec,
                    width: track.width.and_then(|s| parse_numeric(&s)).unwrap_or(0),
                    height: track.height.and_then(|s| parse_numeric(&s)).unwrap_or(0),
                    frame_rate: track.frame_rate.and_then(|s| s.parse().ok()),
                    bit_depth: track.bit_depth.and_then(|s| s.parse().ok()),
                    hdr_format: hdr,
                    dolby_vision: dv,
                    default: track.default.as_deref() == Some("Yes"),
                    language: track.language,
                });
            }
            "Audio" => {
                let codec_str = track.format.clone().unwrap_or_default();
                let atmos = codec_str.to_lowercase().contains("atmos")
                    || track
                        .title
                        .as_ref()
                        .is_some_and(|t| t.to_lowercase().contains("atmos"));
                let codec = map_audio_codec(&codec_str);

                info.audio_tracks.push(AudioTrack {
                    codec,
                    channels: track.channels.and_then(|s| parse_numeric(&s)).unwrap_or(2),
                    sample_rate: track.sample_rate.and_then(|s| parse_numeric(&s)),
                    language: track.language,
                    atmos,
                    default: track.default.as_deref() == Some("Yes"),
                });
            }
            "Text" => {
                info.subtitle_tracks.push(SubtitleTrack {
                    codec: track.format.unwrap_or_default(),
                    language: track.language,
                    forced: track.forced.as_deref() == Some("Yes"),
                    default: track.default.as_deref() == Some("Yes"),
                });
            }
            _ => {}
        }
    }

    Ok(info)
}

fn parse_numeric<T: std::str::FromStr>(s: &str) -> Option<T> {
    s.split_whitespace()
        .next()
        .and_then(|n| n.replace(' ', "").parse().ok())
}

fn parse_hdr_format(hdr_str: Option<&str>) -> HdrFormat {
    let Some(s) = hdr_str else {
        return HdrFormat::Sdr;
    };
    let lower = s.to_lowercase();
    if lower.contains("dolby vision") {
        HdrFormat::DolbyVision
    } else if lower.contains("hdr10+") || lower.contains("hdr10 plus") {
        HdrFormat::Hdr10Plus
    } else if lower.contains("hdr10") || lower.contains("smpte st 2086") {
        HdrFormat::Hdr10
    } else if lower.contains("hlg") {
        HdrFormat::Hlg
    } else {
        HdrFormat::Sdr
    }
}

fn parse_dolby_vision(track: &MiTrack) -> Option<DvInfo> {
    let hdr_str = track.hdr_format.as_ref()?;
    if !hdr_str.to_lowercase().contains("dolby vision") {
        return None;
    }

    // Parse profile from HDR format string like
    // "Dolby Vision, Version 1.0, dvhe.07.06, BL+EL+RPU"
    let profile = hdr_str
        .split(',')
        .find(|s| s.contains("dvhe.") || s.contains("dvav."))
        .and_then(|s| s.trim().split('.').nth(1).and_then(|p| p.parse::<u8>().ok()))
        .unwrap_or(0);

    let components = hdr_str.to_lowercase();

    Some(DvInfo {
        profile,
        rpu_present: components.contains("rpu"),
        el_present: components.contains("el"),
        bl_present: components.contains("bl"),
    })
}

fn map_container(format_name: &str) -> Container {
    let lower = format_name.to_lowercase();
    if lower.contains("matroska") || lower.contains("mkv") || lower.contains("webm") {
        Container::Mkv
    } else {
        Container::Mp4
    }
}

fn map_video_codec(format: &str) -> VideoCodec {
    let lower = format.to_lowercase();
    if lower.contains("avc") || lower.contains("h264") || lower == "h.264" {
        VideoCodec::H264
    } else if lower.contains("hevc") || lower.contains("h265") || lower == "h.265" {
        VideoCodec::H265
    } else if lower.contains("av1") {
        VideoCodec::Av1
    } else if lower.contains("vp9") {
        VideoCodec::Vp9
    } else {
        VideoCodec::H265
    }
}

fn map_audio_codec(format: &str) -> AudioCodec {
    let lower = format.to_lowercase();
    if lower.contains("aac") {
        AudioCodec::Aac
    } else if lower == "ac-3" || lower == "ac3" {
        AudioCodec::Ac3
    } else if lower == "e-ac-3" || lower == "eac3" {
        AudioCodec::Eac3
    } else if lower.contains("truehd") || lower.contains("mlp") {
        AudioCodec::TrueHd
    } else if lower.contains("dts-hd") || lower.contains("dtshd") {
        AudioCodec::DtsHd
    } else if lower.contains("dts") {
        AudioCodec::Dts
    } else if lower.contains("flac") {
        AudioCodec::Flac
    } else if lower.contains("opus") {
        AudioCodec::Opus
    } else {
        AudioCodec::Aac
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_numeric_with_units() {
        assert_eq!(parse_numeric::<u32>("1920 pixels"), Some(1920));
        assert_eq!(parse_numeric::<u32>("1080"), Some(1080));
        assert_eq!(parse_numeric::<u32>("6 channels"), Some(6));
    }

    #[test]
    fn hdr_format_detection() {
        assert_eq!(
            parse_hdr_format(Some("Dolby Vision, Version 1.0")),
            HdrFormat::DolbyVision
        );
        assert_eq!(
            parse_hdr_format(Some("SMPTE ST 2086, HDR10")),
            HdrFormat::Hdr10
        );
        assert_eq!(parse_hdr_format(Some("HDR10+")), HdrFormat::Hdr10Plus);
        assert_eq!(parse_hdr_format(None), HdrFormat::Sdr);
    }

    #[test]
    fn container_detection() {
        assert_eq!(map_container("Matroska"), Container::Mkv);
        assert_eq!(map_container("MPEG-4"), Container::Mp4);
    }
}
