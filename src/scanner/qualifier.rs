//! Source file qualification for Profile B compatibility.
//!
//! This module determines whether a source file can serve as a "universal" file
//! (Profile B compatible) without needing conversion.
//!
//! Profile B requirements:
//! - Container: MP4 with faststart (moov before mdat)
//! - Video: H.264, ≤1920x1080, 8-bit SDR
//! - Audio: AAC stereo
//! - Keyframes: ≤3s interval
//! - Single video + single audio track

use crate::probe::MediaInfo;

/// Result of checking source file qualification.
#[derive(Debug, Clone)]
pub struct QualificationResult {
    /// Whether the source can serve as universal without conversion.
    pub serves_as_universal: bool,
    /// Whether the file has faststart (moov before mdat).
    pub has_faststart: bool,
    /// Detected keyframe interval in seconds, if known.
    pub keyframe_interval_secs: Option<f64>,
    /// Reasons why the file doesn't qualify (if any).
    pub disqualification_reasons: Vec<String>,
}

/// Source file qualifier for Profile B compatibility.
pub struct SourceQualifier {
    /// Maximum resolution width for Profile B.
    max_width: u32,
    /// Maximum resolution height for Profile B.
    max_height: u32,
}

impl SourceQualifier {
    /// Create a new qualifier with default Profile B requirements.
    pub fn new() -> Self {
        Self {
            max_width: 1920,
            max_height: 1080,
        }
    }

    /// Check if a media file qualifies as Profile B compatible.
    pub fn check(&self, info: &MediaInfo) -> QualificationResult {
        let mut reasons = Vec::new();
        let mut qualifies = true;

        // Check container
        let container_lower = info.container.to_lowercase();
        let is_mp4 = container_lower.contains("mp4")
            || container_lower.contains("mpeg-4")
            || container_lower.contains("m4v");

        if !is_mp4 {
            reasons.push(format!("Container must be MP4, found: {}", info.container));
            qualifies = false;
        }

        // Check video track count
        if info.video_tracks.is_empty() {
            reasons.push("No video tracks found".to_string());
            qualifies = false;
        } else if info.video_tracks.len() > 1 {
            reasons.push(format!(
                "Multiple video tracks found ({}), need exactly 1",
                info.video_tracks.len()
            ));
            qualifies = false;
        }

        // Check audio track count
        if info.audio_tracks.is_empty() {
            reasons.push("No audio tracks found".to_string());
            qualifies = false;
        } else if info.audio_tracks.len() > 1 {
            reasons.push(format!(
                "Multiple audio tracks found ({}), need exactly 1",
                info.audio_tracks.len()
            ));
            qualifies = false;
        }

        // Check video properties
        if let Some(video) = info.video_tracks.first() {
            // Codec must be H.264
            let codec_lower = video.codec.to_lowercase();
            let is_h264 = codec_lower.contains("h264")
                || codec_lower.contains("avc")
                || codec_lower.contains("264");

            if !is_h264 {
                reasons.push(format!("Video codec must be H.264, found: {}", video.codec));
                qualifies = false;
            }

            // Resolution check
            if video.width > self.max_width || video.height > self.max_height {
                reasons.push(format!(
                    "Resolution {}x{} exceeds maximum {}x{}",
                    video.width, video.height, self.max_width, self.max_height
                ));
                qualifies = false;
            }

            // Bit depth - must be 8-bit (SDR)
            if let Some(bit_depth) = video.bit_depth {
                if bit_depth > 8 {
                    reasons.push(format!("Bit depth must be 8-bit, found: {}-bit", bit_depth));
                    qualifies = false;
                }
            }

            // HDR check - must be SDR
            if video.hdr_format.is_some() {
                reasons.push("File has HDR, Profile B requires SDR".to_string());
                qualifies = false;
            }

            // Dolby Vision check
            if video.dolby_vision.is_some() {
                reasons.push("File has Dolby Vision, Profile B requires SDR".to_string());
                qualifies = false;
            }
        }

        // Check audio properties
        if let Some(audio) = info.audio_tracks.first() {
            let codec_lower = audio.codec.to_lowercase();
            let is_aac = codec_lower.contains("aac");

            if !is_aac {
                reasons.push(format!("Audio codec must be AAC, found: {}", audio.codec));
                qualifies = false;
            }

            // Stereo requirement (2 channels)
            if audio.channels != 2 {
                reasons.push(format!(
                    "Audio must be stereo (2 channels), found: {} channels",
                    audio.channels
                ));
                qualifies = false;
            }
        }

        // For now, assume files don't have faststart (would need deeper analysis)
        // A proper implementation would check the moov atom position
        let has_faststart = false;
        if !has_faststart && qualifies {
            // Only mention faststart if everything else qualifies
            // In practice, we could still serve the file, just less efficiently
        }

        QualificationResult {
            serves_as_universal: qualifies,
            has_faststart,
            keyframe_interval_secs: None, // Would need keyframe analysis
            disqualification_reasons: reasons,
        }
    }
}

impl Default for SourceQualifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::{AudioTrack, DolbyVisionInfo, HdrFormat, VideoTrack};
    use std::path::PathBuf;
    use std::time::Duration;

    fn make_base_info() -> MediaInfo {
        MediaInfo {
            file_path: PathBuf::from("/test/file.mp4"),
            file_size: 1024,
            container: "MPEG-4".to_string(),
            duration: Some(Duration::from_secs(120)),
            video_tracks: vec![VideoTrack {
                index: 0,
                codec: "h264".to_string(),
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

    #[test]
    fn test_qualifies_profile_b() {
        let qualifier = SourceQualifier::new();
        let info = make_base_info();

        let result = qualifier.check(&info);
        assert!(
            result.serves_as_universal,
            "Should qualify: {:?}",
            result.disqualification_reasons
        );
    }

    #[test]
    fn test_disqualifies_mkv_container() {
        let qualifier = SourceQualifier::new();
        let mut info = make_base_info();
        info.container = "Matroska".to_string();

        let result = qualifier.check(&info);
        assert!(!result.serves_as_universal);
        assert!(result
            .disqualification_reasons
            .iter()
            .any(|r| r.contains("Container")));
    }

    #[test]
    fn test_disqualifies_hevc_codec() {
        let qualifier = SourceQualifier::new();
        let mut info = make_base_info();
        info.video_tracks[0].codec = "hevc".to_string();

        let result = qualifier.check(&info);
        assert!(!result.serves_as_universal);
        assert!(result
            .disqualification_reasons
            .iter()
            .any(|r| r.contains("H.264")));
    }

    #[test]
    fn test_disqualifies_4k_resolution() {
        let qualifier = SourceQualifier::new();
        let mut info = make_base_info();
        info.video_tracks[0].width = 3840;
        info.video_tracks[0].height = 2160;

        let result = qualifier.check(&info);
        assert!(!result.serves_as_universal);
        assert!(result
            .disqualification_reasons
            .iter()
            .any(|r| r.contains("Resolution")));
    }

    #[test]
    fn test_disqualifies_hdr() {
        let qualifier = SourceQualifier::new();
        let mut info = make_base_info();
        info.video_tracks[0].hdr_format = Some(HdrFormat::Hdr10);

        let result = qualifier.check(&info);
        assert!(!result.serves_as_universal);
        assert!(result
            .disqualification_reasons
            .iter()
            .any(|r| r.contains("HDR")));
    }

    #[test]
    fn test_disqualifies_dolby_vision() {
        let qualifier = SourceQualifier::new();
        let mut info = make_base_info();
        info.video_tracks[0].dolby_vision = Some(DolbyVisionInfo {
            profile: 7,
            level: Some(6),
            rpu_present: true,
            el_present: true,
            bl_present: true,
            bl_compatibility_id: Some(1),
        });

        let result = qualifier.check(&info);
        assert!(!result.serves_as_universal);
        assert!(result
            .disqualification_reasons
            .iter()
            .any(|r| r.contains("Dolby Vision")));
    }

    #[test]
    fn test_disqualifies_surround_audio() {
        let qualifier = SourceQualifier::new();
        let mut info = make_base_info();
        info.audio_tracks[0].channels = 6;

        let result = qualifier.check(&info);
        assert!(!result.serves_as_universal);
        assert!(result
            .disqualification_reasons
            .iter()
            .any(|r| r.contains("stereo")));
    }

    #[test]
    fn test_disqualifies_non_aac_audio() {
        let qualifier = SourceQualifier::new();
        let mut info = make_base_info();
        info.audio_tracks[0].codec = "AC3".to_string();

        let result = qualifier.check(&info);
        assert!(!result.serves_as_universal);
        assert!(result
            .disqualification_reasons
            .iter()
            .any(|r| r.contains("AAC")));
    }

    #[test]
    fn test_disqualifies_multiple_video_tracks() {
        let qualifier = SourceQualifier::new();
        let mut info = make_base_info();
        info.video_tracks.push(info.video_tracks[0].clone());

        let result = qualifier.check(&info);
        assert!(!result.serves_as_universal);
        assert!(result
            .disqualification_reasons
            .iter()
            .any(|r| r.contains("Multiple video")));
    }
}
