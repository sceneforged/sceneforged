//! FFprobe-based [`sf_probe::Prober`] implementation.
//!
//! Shells out to `ffprobe -v quiet -print_format json -show_format -show_streams`
//! and maps the JSON output into [`sf_probe::MediaInfo`].

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use sf_core::{AudioCodec, Container, HdrFormat, VideoCodec};
use sf_probe::types::{AudioTrack, DvInfo, MediaInfo, SubtitleTrack, VideoTrack};
use sf_probe::Prober;

use crate::command::ToolCommand;

/// A prober backed by the `ffprobe` CLI.
#[derive(Debug, Clone)]
pub struct FfprobeProber {
    /// Path to the ffprobe binary.
    ffprobe_path: PathBuf,
}

impl FfprobeProber {
    /// Create a new prober using the given ffprobe path.
    pub fn new(ffprobe_path: PathBuf) -> Self {
        Self { ffprobe_path }
    }

    /// Create a prober that finds ffprobe on `PATH`.
    pub fn from_path() -> Option<Self> {
        which::which("ffprobe")
            .ok()
            .map(|p| Self { ffprobe_path: p })
    }
}

impl Prober for FfprobeProber {
    fn name(&self) -> &'static str {
        "ffprobe"
    }

    fn probe(&self, path: &Path) -> sf_core::Result<MediaInfo> {
        // Run synchronously by creating a small tokio runtime.
        // The Prober trait is sync, but we use ToolCommand internally.
        let rt = tokio::runtime::Handle::try_current();
        match rt {
            Ok(handle) => {
                // We are inside a tokio runtime already; use block_in_place.
                tokio::task::block_in_place(|| {
                    handle.block_on(self.probe_async(path))
                })
            }
            Err(_) => {
                // No runtime active; create a temporary one.
                let rt = tokio::runtime::Runtime::new().map_err(|e| sf_core::Error::Tool {
                    tool: "ffprobe".into(),
                    message: format!("failed to create tokio runtime: {e}"),
                })?;
                rt.block_on(self.probe_async(path))
            }
        }
    }

    fn supports(&self, path: &Path) -> bool {
        // ffprobe supports basically all media formats.
        path.extension().is_some()
    }
}

impl FfprobeProber {
    async fn probe_async(&self, path: &Path) -> sf_core::Result<MediaInfo> {
        let mut cmd = ToolCommand::new(self.ffprobe_path.clone());
        cmd.args([
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
        ]);
        cmd.arg(path.to_string_lossy().as_ref());

        let output = cmd.execute().await?;
        let ff: FfprobeOutput = serde_json::from_str(&output.stdout).map_err(|e| {
            sf_core::Error::Probe(format!("ffprobe JSON parse error: {e}"))
        })?;

        parse_ffprobe_output(path, ff)
    }
}

// ---------------------------------------------------------------------------
// JSON structures
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    format: FfprobeFormat,
    #[serde(default)]
    streams: Vec<FfprobeStream>,
}

#[derive(Debug, Deserialize)]
struct FfprobeFormat {
    format_name: Option<String>,
    duration: Option<String>,
    size: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    r_frame_rate: Option<String>,
    bits_per_raw_sample: Option<String>,
    channels: Option<u32>,
    sample_rate: Option<String>,
    #[serde(default)]
    disposition: FfprobeDisposition,
    #[serde(default)]
    tags: FfprobeTags,
    color_primaries: Option<String>,
    color_transfer: Option<String>,
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
}

#[derive(Debug, Deserialize)]
struct FfprobeSideData {
    side_data_type: Option<String>,
    dv_profile: Option<u8>,
    rpu_present_flag: Option<u8>,
    el_present_flag: Option<u8>,
    bl_present_flag: Option<u8>,
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

fn parse_ffprobe_output(path: &Path, output: FfprobeOutput) -> sf_core::Result<MediaInfo> {
    let duration = output
        .format
        .duration
        .and_then(|s| s.parse::<f64>().ok())
        .map(Duration::from_secs_f64);

    let file_size = output
        .format
        .size
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let container = map_container(
        output.format.format_name.as_deref().unwrap_or(""),
    );

    let mut video_tracks = Vec::new();
    let mut audio_tracks = Vec::new();
    let mut subtitle_tracks = Vec::new();

    for stream in output.streams {
        let codec_type = stream.codec_type.as_deref().unwrap_or("");
        match codec_type {
            "video" => {
                let (hdr_format, dv_info) = detect_hdr(&stream);
                let codec = map_video_codec(stream.codec_name.as_deref().unwrap_or(""));
                let bit_depth = stream
                    .bits_per_raw_sample
                    .as_deref()
                    .and_then(|s| s.parse::<u8>().ok());

                video_tracks.push(VideoTrack {
                    codec,
                    width: stream.width.unwrap_or(0),
                    height: stream.height.unwrap_or(0),
                    frame_rate: stream.r_frame_rate.and_then(|s| parse_frame_rate(&s)),
                    bit_depth,
                    hdr_format,
                    dolby_vision: dv_info,
                    default: stream.disposition.default == 1,
                    language: stream.tags.language,
                });
            }
            "audio" => {
                let codec = map_audio_codec(stream.codec_name.as_deref().unwrap_or(""));
                audio_tracks.push(AudioTrack {
                    codec,
                    channels: stream.channels.unwrap_or(2),
                    sample_rate: stream.sample_rate.and_then(|s| s.parse().ok()),
                    language: stream.tags.language,
                    atmos: false,
                    default: stream.disposition.default == 1,
                });
            }
            "subtitle" => {
                subtitle_tracks.push(SubtitleTrack {
                    codec: stream.codec_name.unwrap_or_default().to_uppercase(),
                    language: stream.tags.language,
                    forced: stream.disposition.forced == 1,
                    default: stream.disposition.default == 1,
                });
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

fn detect_hdr(stream: &FfprobeStream) -> (HdrFormat, Option<DvInfo>) {
    // Check side-data for Dolby Vision configuration record.
    for sd in &stream.side_data_list {
        if sd.side_data_type.as_deref() == Some("DOVI configuration record") {
            let dv = DvInfo {
                profile: sd.dv_profile.unwrap_or(0),
                rpu_present: sd.rpu_present_flag == Some(1),
                el_present: sd.el_present_flag == Some(1),
                bl_present: sd.bl_present_flag == Some(1),
            };
            return (HdrFormat::DolbyVision, Some(dv));
        }
    }

    // Check color properties for HDR10 / HLG.
    let primaries = stream.color_primaries.as_deref().unwrap_or("");
    let transfer = stream.color_transfer.as_deref().unwrap_or("");

    if primaries == "bt2020" || primaries == "bt2020nc" {
        if transfer == "arib-std-b67" {
            return (HdrFormat::Hlg, None);
        }
        if transfer == "smpte2084" {
            return (HdrFormat::Hdr10, None);
        }
    }

    (HdrFormat::Sdr, None)
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

fn map_container(format_name: &str) -> Container {
    let lower = format_name.to_lowercase();
    if lower.contains("matroska") || lower.contains("webm") {
        Container::Mkv
    } else {
        Container::Mp4
    }
}

fn map_video_codec(codec_name: &str) -> VideoCodec {
    match codec_name {
        "h264" | "avc" => VideoCodec::H264,
        "hevc" | "h265" => VideoCodec::H265,
        "av1" => VideoCodec::Av1,
        "vp9" => VideoCodec::Vp9,
        _ => VideoCodec::H265, // fallback
    }
}

fn map_audio_codec(codec_name: &str) -> AudioCodec {
    match codec_name {
        "aac" => AudioCodec::Aac,
        "ac3" => AudioCodec::Ac3,
        "eac3" => AudioCodec::Eac3,
        "truehd" => AudioCodec::TrueHd,
        "dts" => AudioCodec::Dts,
        "dts-hd" | "dtshd" => AudioCodec::DtsHd,
        "flac" => AudioCodec::Flac,
        "opus" => AudioCodec::Opus,
        _ => AudioCodec::Aac, // fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_rate_fraction() {
        assert!((parse_frame_rate("24000/1001").unwrap() - 23.976).abs() < 0.01);
        assert_eq!(parse_frame_rate("30/1"), Some(30.0));
        assert_eq!(parse_frame_rate("25"), Some(25.0));
        assert_eq!(parse_frame_rate("invalid"), None);
    }

    #[test]
    fn container_mapping() {
        assert_eq!(map_container("matroska,webm"), Container::Mkv);
        assert_eq!(map_container("mov,mp4,m4a,3gp"), Container::Mp4);
    }

    #[test]
    fn video_codec_mapping() {
        assert_eq!(map_video_codec("h264"), VideoCodec::H264);
        assert_eq!(map_video_codec("hevc"), VideoCodec::H265);
        assert_eq!(map_video_codec("av1"), VideoCodec::Av1);
        assert_eq!(map_video_codec("vp9"), VideoCodec::Vp9);
    }

    #[test]
    fn audio_codec_mapping() {
        assert_eq!(map_audio_codec("aac"), AudioCodec::Aac);
        assert_eq!(map_audio_codec("ac3"), AudioCodec::Ac3);
        assert_eq!(map_audio_codec("truehd"), AudioCodec::TrueHd);
        assert_eq!(map_audio_codec("flac"), AudioCodec::Flac);
    }

    #[test]
    fn hdr_detection_sdr() {
        let stream = FfprobeStream {
            codec_type: Some("video".into()),
            codec_name: Some("hevc".into()),
            width: Some(1920),
            height: Some(1080),
            r_frame_rate: None,
            bits_per_raw_sample: None,
            channels: None,
            sample_rate: None,
            disposition: FfprobeDisposition::default(),
            tags: FfprobeTags::default(),
            color_primaries: None,
            color_transfer: None,
            side_data_list: vec![],
        };
        let (hdr, dv) = detect_hdr(&stream);
        assert_eq!(hdr, HdrFormat::Sdr);
        assert!(dv.is_none());
    }

    #[test]
    fn hdr_detection_dolby_vision() {
        let stream = FfprobeStream {
            codec_type: Some("video".into()),
            codec_name: Some("hevc".into()),
            width: Some(3840),
            height: Some(2160),
            r_frame_rate: None,
            bits_per_raw_sample: None,
            channels: None,
            sample_rate: None,
            disposition: FfprobeDisposition::default(),
            tags: FfprobeTags::default(),
            color_primaries: Some("bt2020".into()),
            color_transfer: Some("smpte2084".into()),
            side_data_list: vec![FfprobeSideData {
                side_data_type: Some("DOVI configuration record".into()),
                dv_profile: Some(7),
                rpu_present_flag: Some(1),
                el_present_flag: Some(0),
                bl_present_flag: Some(1),
            }],
        };
        let (hdr, dv) = detect_hdr(&stream);
        assert_eq!(hdr, HdrFormat::DolbyVision);
        let dv = dv.unwrap();
        assert_eq!(dv.profile, 7);
        assert!(dv.rpu_present);
        assert!(dv.bl_present);
    }
}
