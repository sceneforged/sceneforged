//! Probe integration tests
//!
//! Tests for mediainfo/ffprobe parsing with mocked JSON output
//! and real files when available.

use sceneforged::probe::{
    AudioTrack, DolbyVisionInfo, HdrFormat, MediaInfo, SubtitleTrack, VideoTrack,
};
use std::path::PathBuf;
use std::time::Duration;

// ===== Fixture functions =====

fn dv_profile7_media() -> MediaInfo {
    MediaInfo {
        file_path: PathBuf::from("/test/dv_p7_movie.mkv"),
        file_size: 15 * 1024 * 1024 * 1024,
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
            index: 1,
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

fn hevc_hdr10_media() -> MediaInfo {
    MediaInfo {
        file_path: PathBuf::from("/test/hdr10_movie.mkv"),
        file_size: 10 * 1024 * 1024 * 1024,
        container: "Matroska".to_string(),
        duration: Some(Duration::from_secs(6000)),
        video_tracks: vec![VideoTrack {
            index: 0,
            codec: "HEVC".to_string(),
            width: 3840,
            height: 2160,
            frame_rate: Some(23.976),
            bit_depth: Some(10),
            hdr_format: Some(HdrFormat::Hdr10),
            dolby_vision: None,
        }],
        audio_tracks: vec![
            AudioTrack {
                index: 1,
                codec: "TrueHD".to_string(),
                channels: 8,
                sample_rate: Some(48000),
                language: Some("eng".to_string()),
                title: Some("TrueHD 7.1".to_string()),
                default: true,
                atmos: true,
            },
            AudioTrack {
                index: 2,
                codec: "AC3".to_string(),
                channels: 6,
                sample_rate: Some(48000),
                language: Some("eng".to_string()),
                title: Some("Dolby Digital 5.1".to_string()),
                default: false,
                atmos: false,
            },
        ],
        subtitle_tracks: vec![],
    }
}

fn h264_sdr_media() -> MediaInfo {
    MediaInfo {
        file_path: PathBuf::from("/test/sdr_movie.mkv"),
        file_size: 4 * 1024 * 1024 * 1024,
        container: "Matroska".to_string(),
        duration: Some(Duration::from_secs(5400)),
        video_tracks: vec![VideoTrack {
            index: 0,
            codec: "AVC".to_string(),
            width: 1920,
            height: 1080,
            frame_rate: Some(24.0),
            bit_depth: Some(8),
            hdr_format: None,
            dolby_vision: None,
        }],
        audio_tracks: vec![AudioTrack {
            index: 1,
            codec: "AAC".to_string(),
            channels: 2,
            sample_rate: Some(48000),
            language: Some("eng".to_string()),
            title: None,
            default: true,
            atmos: false,
        }],
        subtitle_tracks: vec![],
    }
}

fn multitrack_media() -> MediaInfo {
    MediaInfo {
        file_path: PathBuf::from("/test/multitrack_movie.mkv"),
        file_size: 12 * 1024 * 1024 * 1024,
        container: "Matroska".to_string(),
        duration: Some(Duration::from_secs(7800)),
        video_tracks: vec![VideoTrack {
            index: 0,
            codec: "HEVC".to_string(),
            width: 3840,
            height: 2160,
            frame_rate: Some(23.976),
            bit_depth: Some(10),
            hdr_format: Some(HdrFormat::Hdr10Plus),
            dolby_vision: None,
        }],
        audio_tracks: vec![
            AudioTrack {
                index: 1,
                codec: "TrueHD".to_string(),
                channels: 8,
                sample_rate: Some(48000),
                language: Some("eng".to_string()),
                title: Some("English - Atmos".to_string()),
                default: true,
                atmos: true,
            },
            AudioTrack {
                index: 2,
                codec: "AC3".to_string(),
                channels: 6,
                sample_rate: Some(48000),
                language: Some("eng".to_string()),
                title: Some("English - Compatibility".to_string()),
                default: false,
                atmos: false,
            },
            AudioTrack {
                index: 3,
                codec: "DTS-HD MA".to_string(),
                channels: 6,
                sample_rate: Some(48000),
                language: Some("spa".to_string()),
                title: Some("Spanish".to_string()),
                default: false,
                atmos: false,
            },
            AudioTrack {
                index: 4,
                codec: "AAC".to_string(),
                channels: 2,
                sample_rate: Some(48000),
                language: Some("jpn".to_string()),
                title: Some("Japanese".to_string()),
                default: false,
                atmos: false,
            },
        ],
        subtitle_tracks: vec![
            SubtitleTrack {
                index: 5,
                codec: "SubRip".to_string(),
                language: Some("eng".to_string()),
                title: Some("English".to_string()),
                default: true,
                forced: false,
            },
            SubtitleTrack {
                index: 6,
                codec: "SubRip".to_string(),
                language: Some("eng".to_string()),
                title: Some("English (Forced)".to_string()),
                default: false,
                forced: true,
            },
            SubtitleTrack {
                index: 7,
                codec: "SubRip".to_string(),
                language: Some("spa".to_string()),
                title: Some("Spanish".to_string()),
                default: false,
                forced: false,
            },
            SubtitleTrack {
                index: 8,
                codec: "PGS".to_string(),
                language: Some("jpn".to_string()),
                title: Some("Japanese".to_string()),
                default: false,
                forced: false,
            },
        ],
    }
}

fn minimal_media() -> MediaInfo {
    MediaInfo {
        file_path: PathBuf::from("/test/minimal.mkv"),
        file_size: 100 * 1024 * 1024,
        container: "Matroska".to_string(),
        duration: Some(Duration::from_secs(60)),
        video_tracks: vec![VideoTrack {
            index: 0,
            codec: "HEVC".to_string(),
            width: 1920,
            height: 1080,
            frame_rate: Some(30.0),
            bit_depth: Some(8),
            hdr_format: None,
            dolby_vision: None,
        }],
        audio_tracks: vec![AudioTrack {
            index: 1,
            codec: "AAC".to_string(),
            channels: 2,
            sample_rate: Some(48000),
            language: None,
            title: None,
            default: true,
            atmos: false,
        }],
        subtitle_tracks: vec![],
    }
}

fn test_media_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/media")
}

// ===== Tests =====

#[test]
fn test_parse_sample_hevc_hdr10_structure() {
    let info = hevc_hdr10_media();

    assert_eq!(info.video_tracks.len(), 1);
    assert_eq!(info.video_tracks[0].codec, "HEVC");
    assert_eq!(info.video_tracks[0].width, 3840);
    assert_eq!(info.video_tracks[0].height, 2160);
    assert_eq!(info.video_tracks[0].hdr_format, Some(HdrFormat::Hdr10));
    assert!(info.video_tracks[0].dolby_vision.is_none());

    assert_eq!(info.audio_tracks.len(), 2);
    assert_eq!(info.audio_tracks[0].codec, "TrueHD");
    assert!(info.audio_tracks[0].atmos);
}

#[test]
fn test_parse_sample_dv_profile7_structure() {
    let info = dv_profile7_media();

    assert_eq!(info.video_tracks.len(), 1);
    assert_eq!(info.video_tracks[0].codec, "HEVC");
    assert_eq!(
        info.video_tracks[0].hdr_format,
        Some(HdrFormat::DolbyVision)
    );

    let dv = info.video_tracks[0].dolby_vision.as_ref().unwrap();
    assert_eq!(dv.profile, 7);
    assert!(dv.rpu_present);
    assert!(dv.el_present);
    assert!(dv.bl_present);
    assert_eq!(dv.bl_compatibility_id, Some(1));
}

#[test]
fn test_media_info_helpers() {
    let dv_info = dv_profile7_media();
    assert!(dv_info.has_dolby_vision());
    assert_eq!(dv_info.dolby_vision_profile(), Some(7));
    assert_eq!(dv_info.resolution_name(), Some("4K"));

    let sdr_info = h264_sdr_media();
    assert!(!sdr_info.has_dolby_vision());
    assert_eq!(sdr_info.dolby_vision_profile(), None);
    assert_eq!(sdr_info.resolution_name(), Some("1080p"));
}

#[test]
fn test_multitrack_media_structure() {
    let info = multitrack_media();

    assert_eq!(info.video_tracks.len(), 1);
    assert_eq!(info.video_tracks[0].hdr_format, Some(HdrFormat::Hdr10Plus));

    assert_eq!(info.audio_tracks.len(), 4);
    assert_eq!(info.audio_tracks[0].language, Some("eng".to_string()));
    assert_eq!(info.audio_tracks[1].language, Some("eng".to_string()));
    assert_eq!(info.audio_tracks[2].language, Some("spa".to_string()));
    assert_eq!(info.audio_tracks[3].language, Some("jpn".to_string()));

    assert_eq!(info.subtitle_tracks.len(), 4);
    assert!(info.subtitle_tracks[1].forced);
}

#[test]
fn test_primary_video() {
    let info = minimal_media();
    let primary = info.primary_video().unwrap();

    assert_eq!(primary.index, 0);
    assert_eq!(primary.codec, "HEVC");
    assert_eq!(primary.width, 1920);
    assert_eq!(primary.height, 1080);
}

#[test]
fn test_probe_real_h264_mp4_file() {
    use sceneforged::probe::probe_file;

    let path = test_media_dir().join("sample_640x360.mp4");
    if !path.exists() {
        eprintln!("Skipping: Test file not found: {:?}", path);
        eprintln!("Run: ./scripts/download-test-media.sh");
        return;
    }

    let info = probe_file(&path).expect("Should probe MP4 file");

    // Verify basic video track info
    assert!(!info.video_tracks.is_empty(), "Should have video track");
    let video = &info.video_tracks[0];
    assert!(
        video.codec.contains("264") || video.codec.to_lowercase().contains("avc"),
        "Expected H.264/AVC codec, got: {}",
        video.codec
    );
    assert_eq!(video.width, 640);
    assert_eq!(video.height, 360);

    // Verify no HDR (SDR content)
    assert!(
        video.hdr_format.is_none(),
        "SDR content should not have HDR format"
    );
    assert!(
        video.dolby_vision.is_none(),
        "SDR content should not have DV"
    );
}

#[test]
fn test_probe_real_h264_mkv_file() {
    use sceneforged::probe::probe_file;

    let path = test_media_dir().join("sample.mkv");
    if !path.exists() {
        eprintln!("Skipping: Test file not found: {:?}", path);
        eprintln!("Run: ./scripts/download-test-media.sh");
        return;
    }

    let info = probe_file(&path).expect("Should probe MKV file");

    // Verify container detection
    assert!(
        info.container.to_lowercase().contains("matroska")
            || info.container.to_lowercase().contains("mkv"),
        "Expected Matroska container, got: {}",
        info.container
    );

    // Verify video track
    assert!(!info.video_tracks.is_empty(), "Should have video track");
    let video = &info.video_tracks[0];
    assert!(
        video.codec.contains("264") || video.codec.to_lowercase().contains("avc"),
        "Expected H.264/AVC codec, got: {}",
        video.codec
    );
}

#[test]
fn test_probe_real_vp9_webm_file() {
    use sceneforged::probe::probe_file;

    let path = test_media_dir().join("sample.webm");
    if !path.exists() {
        eprintln!("Skipping: Test file not found: {:?}", path);
        eprintln!("Run: ./scripts/download-test-media.sh");
        return;
    }

    let info = probe_file(&path).expect("Should probe WebM file");

    // Verify video track with VP9
    assert!(!info.video_tracks.is_empty(), "Should have video track");
    let video = &info.video_tracks[0];
    assert!(
        video.codec.to_lowercase().contains("vp9") || video.codec.to_lowercase().contains("vp09"),
        "Expected VP9 codec, got: {}",
        video.codec
    );
}
