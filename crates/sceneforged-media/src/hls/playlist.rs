//! HLS playlist structures.

use crate::segment_map::SegmentMap;
use std::fmt::Write;

/// HLS playlist generator.
#[derive(Debug)]
pub struct HlsPlaylist {
    /// Base URL for segments.
    pub base_url: String,
    /// Media file ID for URL generation.
    pub media_file_id: String,
}

impl HlsPlaylist {
    /// Create a new playlist generator.
    pub fn new(base_url: impl Into<String>, media_file_id: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            media_file_id: media_file_id.into(),
        }
    }

    /// Generate master playlist M3U8.
    pub fn generate_master(&self, streams: &[StreamInfo]) -> String {
        let mut playlist = String::new();

        writeln!(playlist, "#EXTM3U").unwrap();
        writeln!(playlist, "#EXT-X-VERSION:6").unwrap();

        for stream in streams {
            writeln!(
                playlist,
                "#EXT-X-STREAM-INF:BANDWIDTH={},RESOLUTION={}x{},CODECS=\"{}\"",
                stream.bandwidth, stream.width, stream.height, stream.codecs
            )
            .unwrap();
            writeln!(
                playlist,
                "{}/stream/{}/playlist.m3u8",
                self.base_url, stream.id
            )
            .unwrap();
        }

        playlist
    }

    /// Generate media playlist M3U8 from segment map.
    pub fn generate_media(&self, segment_map: &SegmentMap) -> String {
        let mut playlist = String::new();

        writeln!(playlist, "#EXTM3U").unwrap();
        writeln!(playlist, "#EXT-X-VERSION:7").unwrap();
        writeln!(
            playlist,
            "#EXT-X-TARGETDURATION:{}",
            segment_map.max_segment_duration_secs.ceil() as u32
        )
        .unwrap();
        writeln!(playlist, "#EXT-X-MEDIA-SEQUENCE:0").unwrap();
        writeln!(playlist, "#EXT-X-PLAYLIST-TYPE:VOD").unwrap();
        writeln!(playlist, "#EXT-X-INDEPENDENT-SEGMENTS").unwrap();

        // Init segment
        writeln!(
            playlist,
            "#EXT-X-MAP:URI=\"{}/stream/{}/init.mp4\"",
            self.base_url, self.media_file_id
        )
        .unwrap();

        // Segments
        for (i, segment) in segment_map.segments.iter().enumerate() {
            writeln!(playlist, "#EXTINF:{:.6},", segment.duration_secs).unwrap();
            writeln!(
                playlist,
                "{}/stream/{}/segment/{}.m4s",
                self.base_url, self.media_file_id, i
            )
            .unwrap();
        }

        writeln!(playlist, "#EXT-X-ENDLIST").unwrap();

        playlist
    }
}

/// Media playlist for a single rendition.
#[derive(Debug, Clone)]
pub struct MediaPlaylist {
    /// Target duration in seconds.
    pub target_duration: u32,
    /// Media sequence number.
    pub media_sequence: u32,
    /// Playlist type (VOD or EVENT).
    pub playlist_type: PlaylistType,
    /// Init segment URI.
    pub init_uri: Option<String>,
    /// Segment entries.
    pub segments: Vec<SegmentEntry>,
    /// Whether this is an ended playlist.
    pub ended: bool,
}

impl MediaPlaylist {
    /// Create a new VOD playlist.
    pub fn vod() -> Self {
        Self {
            target_duration: 6,
            media_sequence: 0,
            playlist_type: PlaylistType::Vod,
            init_uri: None,
            segments: Vec::new(),
            ended: true,
        }
    }

    /// Create from a segment map.
    pub fn from_segment_map(segment_map: &SegmentMap, base_url: &str, file_id: &str) -> Self {
        let mut playlist = Self::vod();
        playlist.target_duration = segment_map.max_segment_duration_secs.ceil() as u32;
        playlist.init_uri = Some(format!("{}/stream/{}/init.mp4", base_url, file_id));

        for (i, segment) in segment_map.segments.iter().enumerate() {
            playlist.segments.push(SegmentEntry {
                duration: segment.duration_secs,
                uri: format!("{}/stream/{}/segment/{}.m4s", base_url, file_id, i),
                title: None,
                discontinuity: false,
                byte_range: None,
            });
        }

        playlist
    }

    /// Render to M3U8 string.
    pub fn render(&self) -> String {
        let mut out = String::new();

        writeln!(out, "#EXTM3U").unwrap();
        writeln!(out, "#EXT-X-VERSION:7").unwrap();
        writeln!(out, "#EXT-X-TARGETDURATION:{}", self.target_duration).unwrap();
        writeln!(out, "#EXT-X-MEDIA-SEQUENCE:{}", self.media_sequence).unwrap();

        match self.playlist_type {
            PlaylistType::Vod => writeln!(out, "#EXT-X-PLAYLIST-TYPE:VOD").unwrap(),
            PlaylistType::Event => writeln!(out, "#EXT-X-PLAYLIST-TYPE:EVENT").unwrap(),
            PlaylistType::Live => {}
        }

        writeln!(out, "#EXT-X-INDEPENDENT-SEGMENTS").unwrap();

        if let Some(ref init_uri) = self.init_uri {
            writeln!(out, "#EXT-X-MAP:URI=\"{}\"", init_uri).unwrap();
        }

        for segment in &self.segments {
            if segment.discontinuity {
                writeln!(out, "#EXT-X-DISCONTINUITY").unwrap();
            }
            if let Some((offset, length)) = segment.byte_range {
                writeln!(out, "#EXT-X-BYTERANGE:{}@{}", length, offset).unwrap();
            }
            if let Some(ref title) = segment.title {
                writeln!(out, "#EXTINF:{:.6},{}", segment.duration, title).unwrap();
            } else {
                writeln!(out, "#EXTINF:{:.6},", segment.duration).unwrap();
            }
            writeln!(out, "{}", segment.uri).unwrap();
        }

        if self.ended {
            writeln!(out, "#EXT-X-ENDLIST").unwrap();
        }

        out
    }
}

/// Playlist type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaylistType {
    Vod,
    Event,
    Live,
}

/// A segment entry in the playlist.
#[derive(Debug, Clone)]
pub struct SegmentEntry {
    /// Duration in seconds.
    pub duration: f64,
    /// Segment URI.
    pub uri: String,
    /// Optional title.
    pub title: Option<String>,
    /// Discontinuity before this segment.
    pub discontinuity: bool,
    /// Byte range (offset, length).
    pub byte_range: Option<(u64, u64)>,
}

/// Master playlist with multiple renditions.
#[derive(Debug, Clone)]
pub struct MasterPlaylist {
    /// Stream variants.
    pub streams: Vec<StreamInfo>,
}

impl MasterPlaylist {
    /// Create a new master playlist.
    pub fn new() -> Self {
        Self {
            streams: Vec::new(),
        }
    }

    /// Add a stream variant.
    pub fn add_stream(mut self, stream: StreamInfo) -> Self {
        self.streams.push(stream);
        self
    }

    /// Render to M3U8 string.
    pub fn render(&self) -> String {
        let mut out = String::new();

        writeln!(out, "#EXTM3U").unwrap();
        writeln!(out, "#EXT-X-VERSION:6").unwrap();

        for stream in &self.streams {
            write!(
                out,
                "#EXT-X-STREAM-INF:BANDWIDTH={},RESOLUTION={}x{}",
                stream.bandwidth, stream.width, stream.height
            )
            .unwrap();

            if !stream.codecs.is_empty() {
                write!(out, ",CODECS=\"{}\"", stream.codecs).unwrap();
            }

            if let Some(ref frame_rate) = stream.frame_rate {
                write!(out, ",FRAME-RATE={:.3}", frame_rate).unwrap();
            }

            if let Some(ref audio) = stream.audio_group {
                write!(out, ",AUDIO=\"{}\"", audio).unwrap();
            }

            writeln!(out).unwrap();
            writeln!(out, "{}", stream.uri).unwrap();
        }

        out
    }
}

impl Default for MasterPlaylist {
    fn default() -> Self {
        Self::new()
    }
}

/// Stream variant information.
#[derive(Debug, Clone)]
pub struct StreamInfo {
    /// Unique stream ID.
    pub id: String,
    /// Playlist URI.
    pub uri: String,
    /// Bandwidth in bits per second.
    pub bandwidth: u32,
    /// Video width.
    pub width: u32,
    /// Video height.
    pub height: u32,
    /// Codec string (e.g., "avc1.64001f,mp4a.40.2").
    pub codecs: String,
    /// Frame rate.
    pub frame_rate: Option<f64>,
    /// Audio group ID.
    pub audio_group: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment_map::Segment;

    #[test]
    fn test_media_playlist_render() {
        let mut playlist = MediaPlaylist::vod();
        playlist.target_duration = 6;
        playlist.init_uri = Some("/init.mp4".to_string());
        playlist.segments.push(SegmentEntry {
            duration: 5.5,
            uri: "/seg0.m4s".to_string(),
            title: None,
            discontinuity: false,
            byte_range: None,
        });
        playlist.segments.push(SegmentEntry {
            duration: 6.0,
            uri: "/seg1.m4s".to_string(),
            title: None,
            discontinuity: false,
            byte_range: None,
        });

        let m3u8 = playlist.render();

        assert!(m3u8.contains("#EXTM3U"));
        assert!(m3u8.contains("#EXT-X-VERSION:7"));
        assert!(m3u8.contains("#EXT-X-TARGETDURATION:6"));
        assert!(m3u8.contains("#EXT-X-PLAYLIST-TYPE:VOD"));
        assert!(m3u8.contains("#EXT-X-MAP:URI=\"/init.mp4\""));
        assert!(m3u8.contains("#EXTINF:5.500000,"));
        assert!(m3u8.contains("/seg0.m4s"));
        assert!(m3u8.contains("#EXT-X-ENDLIST"));
    }

    #[test]
    fn test_master_playlist_render() {
        let master = MasterPlaylist::new()
            .add_stream(StreamInfo {
                id: "1080p".to_string(),
                uri: "/1080p/playlist.m3u8".to_string(),
                bandwidth: 5000000,
                width: 1920,
                height: 1080,
                codecs: "avc1.64001f,mp4a.40.2".to_string(),
                frame_rate: Some(24.0),
                audio_group: None,
            })
            .add_stream(StreamInfo {
                id: "720p".to_string(),
                uri: "/720p/playlist.m3u8".to_string(),
                bandwidth: 2500000,
                width: 1280,
                height: 720,
                codecs: "avc1.640028,mp4a.40.2".to_string(),
                frame_rate: Some(24.0),
                audio_group: None,
            });

        let m3u8 = master.render();

        assert!(m3u8.contains("#EXTM3U"));
        assert!(m3u8.contains("BANDWIDTH=5000000"));
        assert!(m3u8.contains("RESOLUTION=1920x1080"));
        assert!(m3u8.contains("/1080p/playlist.m3u8"));
    }

    #[test]
    fn test_hls_playlist_generate_media() {
        let segment_map = SegmentMap {
            timescale: 90000,
            duration_secs: 12.0,
            target_duration_secs: 6.0,
            max_segment_duration_secs: 6.0,
            segments: vec![
                Segment {
                    index: 0,
                    start_sample: 0,
                    end_sample: 100,
                    duration_secs: 6.0,
                    start_time_secs: 0.0,
                    byte_ranges: vec![(0, 1000000)],
                    moof_data: None,
                },
                Segment {
                    index: 1,
                    start_sample: 100,
                    end_sample: 200,
                    duration_secs: 6.0,
                    start_time_secs: 6.0,
                    byte_ranges: vec![(1000000, 1000000)],
                    moof_data: None,
                },
            ],
            sample_count: 200,
            init_segment: None,
        };

        let playlist = HlsPlaylist::new("/api", "abc123");
        let m3u8 = playlist.generate_media(&segment_map);

        assert!(m3u8.contains("#EXTM3U"));
        assert!(m3u8.contains("#EXT-X-TARGETDURATION:6"));
        assert!(m3u8.contains("#EXT-X-MAP:URI=\"/api/stream/abc123/init.mp4\""));
        assert!(m3u8.contains("/api/stream/abc123/segment/0.m4s"));
        assert!(m3u8.contains("/api/stream/abc123/segment/1.m4s"));
    }
}
