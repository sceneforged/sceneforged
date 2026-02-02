use crate::probe::{AudioTrack, DolbyVisionInfo, HdrFormat, MediaInfo, VideoTrack};
use std::path::PathBuf;
use std::time::Duration;

pub fn make_test_info() -> MediaInfo {
    MediaInfo {
        file_path: PathBuf::from("/test/movie.mkv"),
        file_size: 1024 * 1024 * 1024,
        container: "Matroska".to_string(),
        duration: Some(Duration::from_secs(7200)),
        video_tracks: vec![VideoTrack {
            index: 0,
            codec: "HEVC".to_string(),
            width: 3840,
            height: 2160,
            frame_rate: Some(23.976),
            bit_depth: Some(10),
            hdr_format: Some(HdrFormat::DolbyVision),
            dolby_vision: Some(DolbyVisionInfo {
                profile: 7,
                level: Some(6),
                rpu_present: true,
                el_present: true,
                bl_present: true,
                bl_compatibility_id: Some(1),
            }),
        }],
        audio_tracks: vec![AudioTrack {
            index: 0,
            codec: "TrueHD".to_string(),
            channels: 8,
            sample_rate: Some(48000),
            language: Some("eng".to_string()),
            title: Some("Dolby Atmos".to_string()),
            default: true,
            atmos: true,
        }],
        subtitle_tracks: vec![],
    }
}

pub fn make_dv_p7_file() -> MediaInfo {
    MediaInfo {
        file_path: PathBuf::from("/test/movie.mkv"),
        file_size: 1024,
        container: "Matroska".to_string(),
        duration: None,
        video_tracks: vec![VideoTrack {
            index: 0,
            codec: "HEVC".to_string(),
            width: 3840,
            height: 2160,
            frame_rate: None,
            bit_depth: Some(10),
            hdr_format: Some(HdrFormat::DolbyVision),
            dolby_vision: Some(DolbyVisionInfo {
                profile: 7,
                level: None,
                rpu_present: true,
                el_present: true,
                bl_present: true,
                bl_compatibility_id: None,
            }),
        }],
        audio_tracks: vec![],
        subtitle_tracks: vec![],
    }
}
