//! Matroska (MKV/WebM) container parsing

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use matroska::Matroska;

use crate::error::VideoProbeError;
use crate::hdr;
use crate::types::{AudioTrack, HdrFormat, MediaInfo, SubtitleTrack, VideoTrack};

/// Probe a Matroska file
pub fn probe(path: &Path) -> Result<MediaInfo, VideoProbeError> {
    let file = File::open(path)?;
    let file_size = file.metadata()?.len();
    let reader = BufReader::new(file);

    let mkv = Matroska::open(reader)
        .map_err(|e| VideoProbeError::ContainerParse(format!("MKV parse error: {}", e)))?;

    let duration_ms = mkv.info.duration.map(|d| d.as_millis() as u64);

    let mut video_tracks = Vec::new();
    let mut audio_tracks = Vec::new();
    let mut subtitle_tracks = Vec::new();

    for (idx, track) in mkv.tracks.iter().enumerate() {
        match &track.settings {
            matroska::Settings::Video(video) => {
                let mut vtrack = VideoTrack {
                    index: idx as u32,
                    codec: codec_id_to_name(&track.codec_id),
                    width: video.pixel_width as u32,
                    height: video.pixel_height as u32,
                    frame_rate: track.default_duration.map(|d| 1.0 / d.as_secs_f64()),
                    bit_depth: None,
                    color_primaries: None,
                    transfer_characteristics: None,
                    matrix_coefficients: None,
                    hdr_format: None,
                    codec_private: track.codec_private.clone(),
                };

                // Try bitstream parsing if we have codec private data
                if let Some(ref codec_private) = vtrack.codec_private {
                    if is_hevc_codec(&track.codec_id) {
                        if let Some(hdr) = hdr::bitstream::detect_hdr_from_hevc(codec_private) {
                            vtrack.hdr_format = Some(hdr);
                        }
                    }
                }

                // Default to SDR if no HDR detected
                if vtrack.hdr_format.is_none() {
                    vtrack.hdr_format = Some(HdrFormat::Sdr);
                }

                video_tracks.push(vtrack);
            }
            matroska::Settings::Audio(audio) => {
                let atrack = AudioTrack {
                    index: idx as u32,
                    codec: codec_id_to_name(&track.codec_id),
                    channels: audio.channels as u8,
                    sample_rate: audio.sample_rate as u32,
                    bit_depth: audio.bit_depth.map(|b| b as u8),
                    language: track.language.as_ref().map(|l| l.to_string()),
                    title: track.name.clone(),
                    default: track.default,
                };
                audio_tracks.push(atrack);
            }
            matroska::Settings::None => {
                // Check if this is a subtitle track by tracktype
                if track.tracktype == matroska::Tracktype::Subtitle {
                    let strack = SubtitleTrack {
                        index: idx as u32,
                        codec: codec_id_to_name(&track.codec_id),
                        language: track.language.as_ref().map(|l| l.to_string()),
                        title: track.name.clone(),
                        default: track.default,
                        forced: track.forced,
                    };
                    subtitle_tracks.push(strack);
                }
            }
        }
    }

    Ok(MediaInfo {
        file_path: path.to_string_lossy().to_string(),
        file_size,
        container: "Matroska".to_string(),
        duration_ms,
        video_tracks,
        audio_tracks,
        subtitle_tracks,
    })
}

/// Check if codec is HEVC/H.265
fn is_hevc_codec(codec_id: &str) -> bool {
    codec_id == "V_MPEGH/ISO/HEVC" || codec_id.contains("HEVC") || codec_id.contains("H265")
}

/// Convert MKV codec ID to human-readable name
fn codec_id_to_name(codec_id: &str) -> String {
    match codec_id {
        // Video codecs
        "V_MPEG4/ISO/AVC" => "AVC".to_string(),
        "V_MPEGH/ISO/HEVC" => "HEVC".to_string(),
        "V_AV1" => "AV1".to_string(),
        "V_VP8" => "VP8".to_string(),
        "V_VP9" => "VP9".to_string(),
        "V_MPEG1" => "MPEG-1".to_string(),
        "V_MPEG2" => "MPEG-2".to_string(),
        "V_MPEG4/ISO/SP" | "V_MPEG4/ISO/ASP" | "V_MPEG4/ISO/AP" => "MPEG-4".to_string(),
        "V_THEORA" => "Theora".to_string(),

        // Audio codecs
        "A_AAC" | "A_AAC/MPEG2/LC" | "A_AAC/MPEG4/LC" | "A_AAC/MPEG4/LC/SBR" => "AAC".to_string(),
        "A_AC3" => "AC-3".to_string(),
        "A_EAC3" => "E-AC-3".to_string(),
        "A_DTS" => "DTS".to_string(),
        "A_DTS/EXPRESS" => "DTS Express".to_string(),
        "A_DTS/LOSSLESS" => "DTS-HD MA".to_string(),
        "A_TRUEHD" => "TrueHD".to_string(),
        "A_FLAC" => "FLAC".to_string(),
        "A_VORBIS" => "Vorbis".to_string(),
        "A_OPUS" => "Opus".to_string(),
        "A_PCM/INT/LIT" | "A_PCM/INT/BIG" => "PCM".to_string(),
        "A_PCM/FLOAT/IEEE" => "PCM Float".to_string(),
        "A_MPEG/L3" => "MP3".to_string(),
        "A_MPEG/L2" => "MP2".to_string(),

        // Subtitle codecs
        "S_TEXT/UTF8" => "SRT".to_string(),
        "S_TEXT/SSA" | "S_TEXT/ASS" => "ASS".to_string(),
        "S_HDMV/PGS" => "PGS".to_string(),
        "S_VOBSUB" => "VobSub".to_string(),
        "S_DVBSUB" => "DVB Subtitle".to_string(),
        "S_TEXT/WEBVTT" => "WebVTT".to_string(),

        // Unknown - return as-is but cleaned up
        other => {
            // Remove common prefixes for cleaner output
            other
                .strip_prefix("V_")
                .or_else(|| other.strip_prefix("A_"))
                .or_else(|| other.strip_prefix("S_"))
                .unwrap_or(other)
                .to_string()
        }
    }
}
