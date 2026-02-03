//! Pure-Rust media file prober implementation.
//!
//! Uses the `matroska` crate for MKV/WebM files and `mp4parse` for MP4/M4V files.
//! Performs best-effort HDR and Dolby Vision detection from codec private data.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::time::Duration;

use sf_core::{AudioCodec, Container, HdrFormat, VideoCodec};

use crate::hdr;
use crate::prober::Prober;
use crate::types::{AudioTrack, DvInfo, MediaInfo, SubtitleTrack, VideoTrack};

/// A pure-Rust [`Prober`] implementation.
///
/// Supports MKV (Matroska), MP4, and M4V files using native Rust parsing crates.
/// No external tools (ffprobe, mediainfo, etc.) are required.
pub struct RustProber;

impl RustProber {
    /// Create a new `RustProber`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for RustProber {
    fn default() -> Self {
        Self::new()
    }
}

impl Prober for RustProber {
    fn name(&self) -> &'static str {
        "rust"
    }

    fn supports(&self, path: &Path) -> bool {
        match path.extension().and_then(|e| e.to_str()) {
            Some(ext) => matches!(ext.to_lowercase().as_str(), "mkv" | "mp4" | "m4v"),
            None => false,
        }
    }

    fn probe(&self, path: &Path) -> sf_core::Result<MediaInfo> {
        let container = detect_container(path)?;
        match container {
            Container::Mkv => probe_mkv(path),
            Container::Mp4 => probe_mp4(path),
        }
    }
}

// ---------------------------------------------------------------------------
// Container detection
// ---------------------------------------------------------------------------

/// Detect container format from file magic bytes, with extension fallback.
fn detect_container(path: &Path) -> sf_core::Result<Container> {
    let mut file = File::open(path).map_err(|e| sf_core::Error::Probe(e.to_string()))?;

    let mut magic = [0u8; 12];
    if file.read(&mut magic).unwrap_or(0) >= 8 {
        // EBML header (Matroska/WebM).
        if magic[0..4] == [0x1A, 0x45, 0xDF, 0xA3] {
            return Ok(Container::Mkv);
        }
        // ftyp box (MP4/MOV).
        if &magic[4..8] == b"ftyp"
            || &magic[4..8] == b"moov"
            || &magic[4..8] == b"mdat"
            || &magic[4..8] == b"free"
        {
            return Ok(Container::Mp4);
        }
    }

    // Fallback to extension.
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => match ext.to_lowercase().as_str() {
            "mkv" | "webm" => Ok(Container::Mkv),
            "mp4" | "m4v" | "mov" => Ok(Container::Mp4),
            other => Err(sf_core::Error::Probe(format!(
                "unsupported container extension: {other}"
            ))),
        },
        None => Err(sf_core::Error::Probe(
            "unable to detect container format".to_string(),
        )),
    }
}

// ---------------------------------------------------------------------------
// MKV probing
// ---------------------------------------------------------------------------

fn probe_mkv(path: &Path) -> sf_core::Result<MediaInfo> {
    let file = File::open(path).map_err(|e| sf_core::Error::Probe(e.to_string()))?;
    let file_size = file
        .metadata()
        .map_err(|e| sf_core::Error::Probe(e.to_string()))?
        .len();
    let reader = BufReader::new(file);

    let mkv = matroska::Matroska::open(reader)
        .map_err(|e| sf_core::Error::Probe(format!("MKV parse error: {e}")))?;

    let duration = mkv.info.duration;

    let mut video_tracks = Vec::new();
    let mut audio_tracks = Vec::new();
    let mut subtitle_tracks = Vec::new();

    for track in &mkv.tracks {
        match &track.settings {
            matroska::Settings::Video(video) => {
                let codec = match mkv_video_codec(&track.codec_id) {
                    Some(c) => c,
                    None => continue, // skip unsupported video codecs
                };

                let frame_rate = track
                    .default_duration
                    .filter(|d| d.as_secs_f64() > 0.0)
                    .map(|d| 1.0 / d.as_secs_f64());

                let mut hdr_format = HdrFormat::Sdr;
                let mut dv: Option<DvInfo> = None;

                // Attempt HDR detection from codec private data for HEVC tracks.
                if codec == VideoCodec::H265 {
                    if let Some(ref private) = track.codec_private {
                        // Try DV config record first.
                        if let Some(dv_info) = hdr::detect_dolby_vision(private) {
                            hdr_format = HdrFormat::DolbyVision;
                            dv = Some(dv_info);
                        } else if let Some(detection) = hdr::detect_hdr_from_hevc(private) {
                            hdr_format = detection.format;
                            dv = detection.dv_info;
                        }
                    }
                }

                let language = track.language.as_ref().map(|l| l.to_string());

                video_tracks.push(VideoTrack {
                    codec,
                    width: video.pixel_width as u32,
                    height: video.pixel_height as u32,
                    frame_rate,
                    bit_depth: None, // matroska crate does not expose bit depth
                    hdr_format,
                    dolby_vision: dv,
                    default: track.default,
                    language,
                });
            }
            matroska::Settings::Audio(audio) => {
                let codec = match mkv_audio_codec(&track.codec_id) {
                    Some(c) => c,
                    None => continue, // skip unsupported audio codecs
                };

                let atmos = is_atmos_hint(&track.codec_id, track.name.as_deref());
                let language = track.language.as_ref().map(|l| l.to_string());

                audio_tracks.push(AudioTrack {
                    codec,
                    channels: audio.channels as u32,
                    sample_rate: Some(audio.sample_rate as u32),
                    language,
                    atmos,
                    default: track.default,
                });
            }
            matroska::Settings::None => {
                if track.tracktype == matroska::Tracktype::Subtitle {
                    let language = track.language.as_ref().map(|l| l.to_string());

                    subtitle_tracks.push(SubtitleTrack {
                        codec: mkv_subtitle_codec(&track.codec_id),
                        language,
                        forced: track.forced,
                        default: track.default,
                    });
                }
            }
        }
    }

    Ok(MediaInfo {
        file_path: path.to_path_buf(),
        file_size,
        container: Container::Mkv,
        duration,
        video_tracks,
        audio_tracks,
        subtitle_tracks,
    })
}

// ---------------------------------------------------------------------------
// MP4 probing
// ---------------------------------------------------------------------------

fn probe_mp4(path: &Path) -> sf_core::Result<MediaInfo> {
    let mut file = File::open(path).map_err(|e| sf_core::Error::Probe(e.to_string()))?;
    let file_size = file
        .metadata()
        .map_err(|e| sf_core::Error::Probe(e.to_string()))?
        .len();

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| sf_core::Error::Probe(e.to_string()))?;

    let mut cursor = std::io::Cursor::new(&buffer);
    let context = mp4parse::read_mp4(&mut cursor)
        .map_err(|e| sf_core::Error::Probe(format!("MP4 parse error: {e:?}")))?;

    let duration = compute_mp4_duration(&context);

    let mut video_tracks = Vec::new();
    let mut audio_tracks = Vec::new();
    let mut subtitle_tracks = Vec::new();

    for (idx, track) in context.tracks.iter().enumerate() {
        match track.track_type {
            mp4parse::TrackType::Video => {
                if let Some(vt) = mp4_video_track(track, idx == 0) {
                    video_tracks.push(vt);
                }
            }
            mp4parse::TrackType::Audio => {
                if let Some(at) = mp4_audio_track(track, idx == 0) {
                    audio_tracks.push(at);
                }
            }
            mp4parse::TrackType::Metadata => {
                if let Some(st) = mp4_subtitle_track(track, idx == 0) {
                    subtitle_tracks.push(st);
                }
            }
            _ => {}
        }
    }

    Ok(MediaInfo {
        file_path: path.to_path_buf(),
        file_size,
        container: Container::Mp4,
        duration,
        video_tracks,
        audio_tracks,
        subtitle_tracks,
    })
}

fn compute_mp4_duration(context: &mp4parse::MediaContext) -> Option<Duration> {
    let global_ts = context.timescale?;
    context.tracks.iter().find_map(|t| {
        let dur = t.duration?;
        let scale = t.timescale.map(|s| s.0).unwrap_or(global_ts.0);
        if scale == 0 {
            return None;
        }
        let ms = dur.0 * 1000 / scale;
        Some(Duration::from_millis(ms))
    })
}

fn mp4_video_track(track: &mp4parse::Track, is_first: bool) -> Option<VideoTrack> {
    let stsd = track.stsd.as_ref()?;
    let entry = stsd.descriptions.first()?;

    let (codec, width, height) = match entry {
        mp4parse::SampleEntry::Video(ve) => {
            let codec = match &ve.codec_specific {
                mp4parse::VideoCodecSpecific::AVCConfig(_) => VideoCodec::H264,
                mp4parse::VideoCodecSpecific::AV1Config(_) => VideoCodec::Av1,
                mp4parse::VideoCodecSpecific::VPxConfig(_) => VideoCodec::Vp9,
                // ESDSConfig and H263Config are not in our supported set.
                _ => return None,
            };
            (codec, ve.width as u32, ve.height as u32)
        }
        _ => return None,
    };

    // mp4parse 0.17 does not expose HEVC configuration, so HDR detection
    // is not possible for MP4 HEVC tracks through this path.
    Some(VideoTrack {
        codec,
        width,
        height,
        frame_rate: None,
        bit_depth: None,
        hdr_format: HdrFormat::Sdr,
        dolby_vision: None,
        default: is_first,
        language: None,
    })
}

fn mp4_audio_track(track: &mp4parse::Track, is_first: bool) -> Option<AudioTrack> {
    let stsd = track.stsd.as_ref()?;
    let entry = stsd.descriptions.first()?;

    let (codec, channels, sample_rate) = match entry {
        mp4parse::SampleEntry::Audio(ae) => {
            let codec = match &ae.codec_specific {
                mp4parse::AudioCodecSpecific::ES_Descriptor(_) => AudioCodec::Aac,
                mp4parse::AudioCodecSpecific::FLACSpecificBox(_) => AudioCodec::Flac,
                mp4parse::AudioCodecSpecific::OpusSpecificBox(_) => AudioCodec::Opus,
                mp4parse::AudioCodecSpecific::ALACSpecificBox(_) => {
                    // ALAC is not in sf_core::AudioCodec; skip.
                    return None;
                }
                mp4parse::AudioCodecSpecific::MP3 => {
                    // MP3 is not in sf_core::AudioCodec; skip.
                    return None;
                }
                mp4parse::AudioCodecSpecific::LPCM => {
                    // LPCM is not in sf_core::AudioCodec; skip.
                    return None;
                }
                #[allow(unreachable_patterns)]
                _ => return None,
            };
            (codec, ae.channelcount, ae.samplerate as u32)
        }
        _ => return None,
    };

    Some(AudioTrack {
        codec,
        channels,
        sample_rate: Some(sample_rate),
        language: None,
        atmos: false,
        default: is_first,
    })
}

fn mp4_subtitle_track(track: &mp4parse::Track, is_first: bool) -> Option<SubtitleTrack> {
    let stsd = track.stsd.as_ref()?;
    let _entry = stsd.descriptions.first()?;

    Some(SubtitleTrack {
        codec: "MP4 Text".to_string(),
        language: None,
        forced: false,
        default: is_first,
    })
}

// ---------------------------------------------------------------------------
// MKV codec mapping
// ---------------------------------------------------------------------------

fn mkv_video_codec(codec_id: &str) -> Option<VideoCodec> {
    match codec_id {
        "V_MPEG4/ISO/AVC" => Some(VideoCodec::H264),
        "V_MPEGH/ISO/HEVC" => Some(VideoCodec::H265),
        "V_AV1" => Some(VideoCodec::Av1),
        "V_VP9" => Some(VideoCodec::Vp9),
        other if other.contains("AVC") || other.contains("H264") => Some(VideoCodec::H264),
        other if other.contains("HEVC") || other.contains("H265") => Some(VideoCodec::H265),
        _ => None,
    }
}

fn mkv_audio_codec(codec_id: &str) -> Option<AudioCodec> {
    match codec_id {
        "A_AAC" | "A_AAC/MPEG2/LC" | "A_AAC/MPEG4/LC" | "A_AAC/MPEG4/LC/SBR" => {
            Some(AudioCodec::Aac)
        }
        "A_AC3" => Some(AudioCodec::Ac3),
        "A_EAC3" => Some(AudioCodec::Eac3),
        "A_DTS" | "A_DTS/EXPRESS" => Some(AudioCodec::Dts),
        "A_DTS/LOSSLESS" => Some(AudioCodec::DtsHd),
        "A_TRUEHD" => Some(AudioCodec::TrueHd),
        "A_FLAC" => Some(AudioCodec::Flac),
        "A_OPUS" => Some(AudioCodec::Opus),
        _ => None,
    }
}

fn mkv_subtitle_codec(codec_id: &str) -> String {
    match codec_id {
        "S_TEXT/UTF8" => "SRT".to_string(),
        "S_TEXT/SSA" | "S_TEXT/ASS" => "ASS".to_string(),
        "S_HDMV/PGS" => "PGS".to_string(),
        "S_VOBSUB" => "VobSub".to_string(),
        "S_DVBSUB" => "DVB Subtitle".to_string(),
        "S_TEXT/WEBVTT" => "WebVTT".to_string(),
        other => other
            .strip_prefix("S_")
            .unwrap_or(other)
            .to_string(),
    }
}

/// Best-effort Atmos detection based on codec ID and track name heuristics.
///
/// True Atmos detection requires inspecting the bitstream, but a track named
/// "Atmos" combined with a TrueHD or E-AC-3 codec is a strong signal.
fn is_atmos_hint(codec_id: &str, track_name: Option<&str>) -> bool {
    let is_atmos_codec = codec_id == "A_TRUEHD" || codec_id == "A_EAC3";
    if !is_atmos_codec {
        return false;
    }
    track_name
        .map(|name| {
            let lower = name.to_lowercase();
            lower.contains("atmos")
        })
        .unwrap_or(false)
}
