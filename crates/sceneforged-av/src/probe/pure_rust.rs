//! Pure Rust probing using sceneforged-probe crate.
//!
//! This module provides media probing without external tools by using
//! the sceneforged-probe crate for container parsing.

use super::types::{
    AudioTrack as AvAudioTrack, DolbyVisionInfo, HdrFormat as AvHdrFormat, MediaInfo as AvMediaInfo,
    SubtitleTrack as AvSubtitleTrack, VideoTrack as AvVideoTrack,
};
use crate::{Error, Result};
use std::path::Path;
use std::time::Duration;

/// Probe a media file using pure Rust parsing.
pub fn probe_with_pure_rust(path: &Path) -> Result<AvMediaInfo> {
    let probe_info = sceneforged_probe::probe_file(path)
        .map_err(|e| Error::parse_error("pure-rust", e.to_string()))?;

    Ok(convert_media_info(path, probe_info))
}

/// Convert sceneforged_probe::MediaInfo to sceneforged_av::MediaInfo.
fn convert_media_info(path: &Path, info: sceneforged_probe::MediaInfo) -> AvMediaInfo {
    AvMediaInfo {
        file_path: path.to_path_buf(),
        file_size: info.file_size,
        container: info.container,
        duration: info.duration_ms.map(Duration::from_millis),
        video_tracks: info
            .video_tracks
            .into_iter()
            .map(convert_video_track)
            .collect(),
        audio_tracks: info
            .audio_tracks
            .into_iter()
            .map(convert_audio_track)
            .collect(),
        subtitle_tracks: info
            .subtitle_tracks
            .into_iter()
            .map(convert_subtitle_track)
            .collect(),
    }
}

/// Convert a video track, mapping HDR format and Dolby Vision info.
fn convert_video_track(track: sceneforged_probe::VideoTrack) -> AvVideoTrack {
    let (hdr_format, dolby_vision) = convert_hdr_format(track.hdr_format);

    AvVideoTrack {
        index: track.index,
        codec: track.codec,
        width: track.width,
        height: track.height,
        frame_rate: track.frame_rate,
        bit_depth: track.bit_depth,
        hdr_format,
        dolby_vision,
    }
}

/// Convert HDR format enum, extracting Dolby Vision info if present.
/// Note: SDR is returned as None to match CLI-based probing behavior.
fn convert_hdr_format(
    hdr: Option<sceneforged_probe::HdrFormat>,
) -> (Option<AvHdrFormat>, Option<DolbyVisionInfo>) {
    match hdr {
        Some(sceneforged_probe::HdrFormat::DolbyVision {
            profile,
            level,
            bl_compatibility_id,
            rpu_present,
            el_present,
            ..
        }) => (
            Some(AvHdrFormat::DolbyVision),
            Some(DolbyVisionInfo {
                profile,
                level,
                rpu_present,
                el_present,
                bl_present: true, // Assumed if we detected DV
                bl_compatibility_id,
            }),
        ),
        Some(sceneforged_probe::HdrFormat::Hdr10 { .. }) => (Some(AvHdrFormat::Hdr10), None),
        Some(sceneforged_probe::HdrFormat::Hdr10Plus { .. }) => (Some(AvHdrFormat::Hdr10Plus), None),
        Some(sceneforged_probe::HdrFormat::Hlg) => (Some(AvHdrFormat::Hlg), None),
        // SDR is returned as None to match CLI-based probing behavior
        Some(sceneforged_probe::HdrFormat::Sdr) | None => (None, None),
    }
}

/// Convert an audio track.
fn convert_audio_track(track: sceneforged_probe::AudioTrack) -> AvAudioTrack {
    AvAudioTrack {
        index: track.index,
        codec: track.codec,
        channels: track.channels as u32, // u8 -> u32
        sample_rate: Some(track.sample_rate),
        language: track.language,
        title: track.title,
        default: track.default,
        atmos: false, // Pure Rust probe doesn't detect Atmos yet
    }
}

/// Convert a subtitle track.
fn convert_subtitle_track(track: sceneforged_probe::SubtitleTrack) -> AvSubtitleTrack {
    AvSubtitleTrack {
        index: track.index,
        codec: track.codec,
        language: track.language,
        title: track.title,
        default: track.default,
        forced: track.forced,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_hdr_format_dolby_vision() {
        let dv = sceneforged_probe::HdrFormat::DolbyVision {
            profile: 8,
            level: Some(6),
            bl_compatibility_id: Some(1),
            rpu_present: true,
            el_present: false,
            bl_signal_compatibility: None,
        };

        let (hdr, dv_info) = convert_hdr_format(Some(dv));
        assert_eq!(hdr, Some(AvHdrFormat::DolbyVision));
        assert!(dv_info.is_some());
        let dv_info = dv_info.unwrap();
        assert_eq!(dv_info.profile, 8);
        assert_eq!(dv_info.level, Some(6));
        assert!(dv_info.rpu_present);
        assert!(!dv_info.el_present);
    }

    #[test]
    fn test_convert_hdr_format_hdr10() {
        let hdr10 = sceneforged_probe::HdrFormat::Hdr10 {
            mastering_display: None,
            content_light_level: None,
        };

        let (hdr, dv_info) = convert_hdr_format(Some(hdr10));
        assert_eq!(hdr, Some(AvHdrFormat::Hdr10));
        assert!(dv_info.is_none());
    }

    #[test]
    fn test_convert_hdr_format_none() {
        let (hdr, dv_info) = convert_hdr_format(None);
        assert!(hdr.is_none());
        assert!(dv_info.is_none());
    }
}
