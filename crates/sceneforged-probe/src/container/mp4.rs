//! MP4/MOV container parsing

use std::fs::File;
use std::io::Read;
use std::path::Path;

use mp4parse::{read_mp4, SampleEntry, TrackType};

use crate::error::VideoProbeError;
use crate::types::{AudioTrack, HdrFormat, MediaInfo, SubtitleTrack, VideoTrack};

/// Probe an MP4 file
pub fn probe(path: &Path) -> Result<MediaInfo, VideoProbeError> {
    let mut file = File::open(path)?;
    let file_size = file.metadata()?.len();

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    let mut cursor = std::io::Cursor::new(&buffer);
    let context = read_mp4(&mut cursor)
        .map_err(|e| VideoProbeError::ContainerParse(format!("MP4 parse error: {:?}", e)))?;

    let duration_ms = context.timescale.and_then(|ts| {
        // Duration is in timescale units
        // We need to find track with duration info
        context.tracks.iter().find_map(|t| {
            t.duration.map(|d| {
                let scale = t.timescale.map(|s| s.0).unwrap_or(ts.0);
                if scale > 0 {
                    d.0 * 1000 / scale
                } else {
                    0
                }
            })
        })
    });

    let mut video_tracks = Vec::new();
    let mut audio_tracks = Vec::new();
    let mut subtitle_tracks = Vec::new();
    let mut video_idx = 0u32;
    let mut audio_idx = 0u32;
    let mut subtitle_idx = 0u32;

    for track in &context.tracks {
        match track.track_type {
            TrackType::Video => {
                if let Some(vtrack) = parse_video_track(track, video_idx) {
                    video_tracks.push(vtrack);
                    video_idx += 1;
                }
            }
            TrackType::Audio => {
                if let Some(atrack) = parse_audio_track(track, audio_idx) {
                    audio_tracks.push(atrack);
                    audio_idx += 1;
                }
            }
            TrackType::Metadata => {
                // Subtitle tracks in MP4 are often metadata type
                if let Some(strack) = parse_subtitle_track(track, subtitle_idx) {
                    subtitle_tracks.push(strack);
                    subtitle_idx += 1;
                }
            }
            _ => {}
        }
    }

    Ok(MediaInfo {
        file_path: path.to_string_lossy().to_string(),
        file_size,
        container: "MP4".to_string(),
        duration_ms,
        video_tracks,
        audio_tracks,
        subtitle_tracks,
    })
}

fn parse_video_track(track: &mp4parse::Track, index: u32) -> Option<VideoTrack> {
    let stsd = track.stsd.as_ref()?;
    let sample_entry = stsd.descriptions.first()?;

    let (codec, width, height) = match sample_entry {
        SampleEntry::Video(ve) => {
            let codec_name = match &ve.codec_specific {
                mp4parse::VideoCodecSpecific::AVCConfig(_) => "AVC".to_string(),
                mp4parse::VideoCodecSpecific::AV1Config(_) => "AV1".to_string(),
                mp4parse::VideoCodecSpecific::VPxConfig(_) => "VP9".to_string(),
                mp4parse::VideoCodecSpecific::ESDSConfig(_) => "MPEG-4".to_string(),
                mp4parse::VideoCodecSpecific::H263Config(_) => "H.263".to_string(),
            };

            (codec_name, ve.width as u32, ve.height as u32)
        }
        _ => return None,
    };

    // MP4 doesn't expose HEVC config directly in mp4parse 0.17
    // For now, we can only detect basic formats
    let vtrack = VideoTrack {
        index,
        codec,
        width,
        height,
        frame_rate: None, // MP4 frame rate requires more calculation
        bit_depth: None,
        color_primaries: None,
        transfer_characteristics: None,
        matrix_coefficients: None,
        hdr_format: Some(HdrFormat::Sdr), // Default to SDR without HEVC parsing
        codec_private: None,
    };

    Some(vtrack)
}

fn parse_audio_track(track: &mp4parse::Track, index: u32) -> Option<AudioTrack> {
    let stsd = track.stsd.as_ref()?;
    let sample_entry = stsd.descriptions.first()?;

    let (codec, channels, sample_rate, bit_depth) = match sample_entry {
        SampleEntry::Audio(ae) => {
            let codec_name = match &ae.codec_specific {
                mp4parse::AudioCodecSpecific::ES_Descriptor(esds) => {
                    // Check audio object type for AAC variants
                    match esds.audio_object_type {
                        Some(1) | Some(2) => "AAC".to_string(),
                        Some(5) => "AAC-SBR".to_string(),
                        Some(29) => "AAC-PS".to_string(),
                        _ => "AAC".to_string(),
                    }
                }
                mp4parse::AudioCodecSpecific::FLACSpecificBox(_) => "FLAC".to_string(),
                mp4parse::AudioCodecSpecific::OpusSpecificBox(_) => "Opus".to_string(),
                mp4parse::AudioCodecSpecific::ALACSpecificBox(_) => "ALAC".to_string(),
                mp4parse::AudioCodecSpecific::MP3 => "MP3".to_string(),
                mp4parse::AudioCodecSpecific::LPCM => "LPCM".to_string(),
                #[allow(unreachable_patterns)]
                _ => "Unknown".to_string(),
            };

            (
                codec_name,
                ae.channelcount as u8,
                ae.samplerate as u32,
                Some(ae.samplesize as u8),
            )
        }
        _ => return None,
    };

    Some(AudioTrack {
        index,
        codec,
        channels,
        sample_rate,
        bit_depth,
        language: None, // Would need to parse mdia/mdhd for language
        title: None,
        default: index == 0, // First track is typically default
    })
}

fn parse_subtitle_track(track: &mp4parse::Track, index: u32) -> Option<SubtitleTrack> {
    let stsd = track.stsd.as_ref()?;
    let _sample_entry = stsd.descriptions.first()?;

    // MP4 subtitle detection is limited
    // Common formats: tx3g (MPEG-4 Timed Text), wvtt (WebVTT)
    Some(SubtitleTrack {
        index,
        codec: "MP4 Text".to_string(),
        language: None,
        title: None,
        default: index == 0,
        forced: false,
    })
}
