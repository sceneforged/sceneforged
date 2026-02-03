//! HLS playlist generation functions.

use super::types::{MasterPlaylist, MediaPlaylist};
use std::fmt::Write;

/// Generate an HLS master playlist (M3U8) from a [`MasterPlaylist`].
///
/// Output includes `#EXTM3U` header and `#EXT-X-STREAM-INF` for each variant.
pub fn generate_master_playlist(playlist: &MasterPlaylist) -> String {
    let mut out = String::new();

    writeln!(out, "#EXTM3U").unwrap();

    for variant in &playlist.variants {
        write!(out, "#EXT-X-STREAM-INF:BANDWIDTH={}", variant.bandwidth).unwrap();

        if let Some((w, h)) = variant.resolution {
            write!(out, ",RESOLUTION={}x{}", w, h).unwrap();
        }

        if !variant.codecs.is_empty() {
            write!(out, ",CODECS=\"{}\"", variant.codecs).unwrap();
        }

        writeln!(out).unwrap();
        writeln!(out, "{}", variant.uri).unwrap();
    }

    out
}

/// Generate an HLS media playlist (M3U8) from a [`MediaPlaylist`].
///
/// Output includes:
/// - `#EXTM3U` header
/// - `#EXT-X-TARGETDURATION`
/// - `#EXT-X-MEDIA-SEQUENCE`
/// - Optional `#EXT-X-MAP` for initialization segment
/// - `#EXTINF` for each segment
/// - Optional `#EXT-X-ENDLIST` for VOD playlists
pub fn generate_media_playlist(playlist: &MediaPlaylist) -> String {
    let mut out = String::new();

    writeln!(out, "#EXTM3U").unwrap();
    writeln!(out, "#EXT-X-VERSION:7").unwrap();
    writeln!(out, "#EXT-X-TARGETDURATION:{}", playlist.target_duration).unwrap();
    writeln!(out, "#EXT-X-MEDIA-SEQUENCE:{}", playlist.media_sequence).unwrap();

    if let Some(ref init_uri) = playlist.init_segment_uri {
        writeln!(out, "#EXT-X-MAP:URI=\"{}\"", init_uri).unwrap();
    }

    for segment in &playlist.segments {
        if let Some(ref title) = segment.title {
            writeln!(out, "#EXTINF:{:.6},{}", segment.duration, title).unwrap();
        } else {
            writeln!(out, "#EXTINF:{:.6},", segment.duration).unwrap();
        }
        writeln!(out, "{}", segment.uri).unwrap();
    }

    if playlist.ended {
        writeln!(out, "#EXT-X-ENDLIST").unwrap();
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hls::types::{Segment, Variant};

    #[test]
    fn test_generate_master_playlist_basic() {
        let playlist = MasterPlaylist {
            variants: vec![
                Variant {
                    bandwidth: 5000000,
                    resolution: Some((1920, 1080)),
                    codecs: "avc1.64001f,mp4a.40.2".to_string(),
                    uri: "1080p/playlist.m3u8".to_string(),
                },
                Variant {
                    bandwidth: 2500000,
                    resolution: Some((1280, 720)),
                    codecs: "avc1.640028,mp4a.40.2".to_string(),
                    uri: "720p/playlist.m3u8".to_string(),
                },
            ],
        };

        let m3u8 = generate_master_playlist(&playlist);

        assert!(m3u8.starts_with("#EXTM3U\n"));
        assert!(m3u8.contains("BANDWIDTH=5000000"));
        assert!(m3u8.contains("RESOLUTION=1920x1080"));
        assert!(m3u8.contains("CODECS=\"avc1.64001f,mp4a.40.2\""));
        assert!(m3u8.contains("1080p/playlist.m3u8"));
        assert!(m3u8.contains("BANDWIDTH=2500000"));
        assert!(m3u8.contains("720p/playlist.m3u8"));
    }

    #[test]
    fn test_generate_master_playlist_no_resolution() {
        let playlist = MasterPlaylist {
            variants: vec![Variant {
                bandwidth: 128000,
                resolution: None,
                codecs: "mp4a.40.2".to_string(),
                uri: "audio/playlist.m3u8".to_string(),
            }],
        };

        let m3u8 = generate_master_playlist(&playlist);

        assert!(m3u8.contains("BANDWIDTH=128000"));
        assert!(!m3u8.contains("RESOLUTION"));
        assert!(m3u8.contains("audio/playlist.m3u8"));
    }

    #[test]
    fn test_generate_master_playlist_empty() {
        let playlist = MasterPlaylist {
            variants: vec![],
        };

        let m3u8 = generate_master_playlist(&playlist);
        assert_eq!(m3u8, "#EXTM3U\n");
    }

    #[test]
    fn test_generate_media_playlist_vod() {
        let playlist = MediaPlaylist {
            target_duration: 6,
            media_sequence: 0,
            segments: vec![
                Segment {
                    duration: 5.5,
                    uri: "seg0.m4s".to_string(),
                    title: None,
                },
                Segment {
                    duration: 6.0,
                    uri: "seg1.m4s".to_string(),
                    title: None,
                },
                Segment {
                    duration: 3.2,
                    uri: "seg2.m4s".to_string(),
                    title: None,
                },
            ],
            ended: true,
            init_segment_uri: Some("init.mp4".to_string()),
        };

        let m3u8 = generate_media_playlist(&playlist);

        assert!(m3u8.starts_with("#EXTM3U\n"));
        assert!(m3u8.contains("#EXT-X-VERSION:7"));
        assert!(m3u8.contains("#EXT-X-TARGETDURATION:6"));
        assert!(m3u8.contains("#EXT-X-MEDIA-SEQUENCE:0"));
        assert!(m3u8.contains("#EXT-X-MAP:URI=\"init.mp4\""));
        assert!(m3u8.contains("#EXTINF:5.500000,"));
        assert!(m3u8.contains("seg0.m4s"));
        assert!(m3u8.contains("#EXTINF:6.000000,"));
        assert!(m3u8.contains("seg1.m4s"));
        assert!(m3u8.contains("#EXTINF:3.200000,"));
        assert!(m3u8.contains("seg2.m4s"));
        assert!(m3u8.contains("#EXT-X-ENDLIST"));
    }

    #[test]
    fn test_generate_media_playlist_live_no_endlist() {
        let playlist = MediaPlaylist {
            target_duration: 4,
            media_sequence: 100,
            segments: vec![Segment {
                duration: 4.0,
                uri: "seg100.m4s".to_string(),
                title: None,
            }],
            ended: false,
            init_segment_uri: None,
        };

        let m3u8 = generate_media_playlist(&playlist);

        assert!(m3u8.contains("#EXT-X-MEDIA-SEQUENCE:100"));
        assert!(!m3u8.contains("#EXT-X-MAP"));
        assert!(!m3u8.contains("#EXT-X-ENDLIST"));
    }

    #[test]
    fn test_generate_media_playlist_with_titles() {
        let playlist = MediaPlaylist {
            target_duration: 10,
            media_sequence: 0,
            segments: vec![
                Segment {
                    duration: 9.5,
                    uri: "seg0.m4s".to_string(),
                    title: Some("Scene 1".to_string()),
                },
                Segment {
                    duration: 8.0,
                    uri: "seg1.m4s".to_string(),
                    title: None,
                },
            ],
            ended: true,
            init_segment_uri: None,
        };

        let m3u8 = generate_media_playlist(&playlist);

        assert!(m3u8.contains("#EXTINF:9.500000,Scene 1"));
        assert!(m3u8.contains("#EXTINF:8.000000,"));
    }

    #[test]
    fn test_media_playlist_format_exact() {
        let playlist = MediaPlaylist {
            target_duration: 6,
            media_sequence: 0,
            segments: vec![Segment {
                duration: 5.0,
                uri: "seg0.m4s".to_string(),
                title: None,
            }],
            ended: true,
            init_segment_uri: Some("init.mp4".to_string()),
        };

        let m3u8 = generate_media_playlist(&playlist);

        let expected = "\
#EXTM3U
#EXT-X-VERSION:7
#EXT-X-TARGETDURATION:6
#EXT-X-MEDIA-SEQUENCE:0
#EXT-X-MAP:URI=\"init.mp4\"
#EXTINF:5.000000,
seg0.m4s
#EXT-X-ENDLIST
";
        assert_eq!(m3u8, expected);
    }
}
