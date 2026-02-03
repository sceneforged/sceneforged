//! Profile classification for media files.
//!
//! This module determines the profile classification (A, B, C) for media files
//! based on their characteristics and conversion eligibility.
//!
//! ## Profile Definitions
//!
//! - **Profile A** (High-quality source): HDR/DV content or 4K HEVC
//! - **Profile B** (Universal playback): MP4, H.264, ≤1080p, SDR, AAC stereo
//! - **Profile C** (Unsupported/Pending): Everything else, needs conversion
//!
//! ## Conversion Eligibility
//!
//! - `can_be_profile_a`: Source has HDR/DV content that can be processed
//! - `can_be_profile_b`: Almost always true (most content can be transcoded)

use crate::probe::MediaInfo;
use sceneforged_common::Profile;

/// Result of profile classification.
#[derive(Debug, Clone)]
pub struct ClassificationResult {
    /// The determined profile for this media file.
    pub profile: Profile,
    /// Whether this file could be converted to Profile A.
    pub can_be_profile_a: bool,
    /// Whether this file could be converted to Profile B.
    pub can_be_profile_b: bool,
}

/// Profile classifier for media files.
pub struct ProfileClassifier;

impl ProfileClassifier {
    /// Create a new profile classifier.
    pub fn new() -> Self {
        Self
    }

    /// Classify a media file into a profile and determine conversion eligibility.
    pub fn classify(&self, info: &MediaInfo) -> ClassificationResult {
        let profile = self.determine_profile(info);
        let can_be_profile_a = self.can_be_profile_a(info);
        let can_be_profile_b = self.can_be_profile_b(info);

        ClassificationResult {
            profile,
            can_be_profile_a,
            can_be_profile_b,
        }
    }

    /// Determine the profile based on media characteristics.
    fn determine_profile(&self, info: &MediaInfo) -> Profile {
        // Check Profile A criteria first (high-quality source)
        if self.qualifies_for_profile_a(info) {
            return Profile::A;
        }

        // Check Profile B criteria (universal playback)
        if self.qualifies_for_profile_b(info) {
            return Profile::B;
        }

        // Default to Profile C (needs conversion)
        Profile::C
    }

    /// Check if a file qualifies for Profile A (high-quality source).
    ///
    /// Profile A criteria:
    /// - Has HDR format (HDR10, HDR10+, HLG, Dolby Vision) OR
    /// - Resolution ≥2160p (4K) AND video codec is HEVC/H.265
    fn qualifies_for_profile_a(&self, info: &MediaInfo) -> bool {
        let video = match info.video_tracks.first() {
            Some(v) => v,
            None => return false,
        };

        // Check for HDR formats
        if let Some(hdr_format) = &video.hdr_format {
            match hdr_format {
                crate::probe::HdrFormat::Hdr10
                | crate::probe::HdrFormat::Hdr10Plus
                | crate::probe::HdrFormat::Hlg
                | crate::probe::HdrFormat::DolbyVision => return true,
                crate::probe::HdrFormat::Sdr => {
                    // SDR, continue to check other criteria
                }
            }
        }

        // Check for Dolby Vision via dolby_vision field
        if video.dolby_vision.is_some() {
            return true;
        }

        // Check for 4K+ HEVC (use relaxed thresholds to catch near-4K crops like 3822x2066)
        let is_4k = video.width >= 3600 || video.height >= 2000;
        let is_hevc = self.is_hevc_codec(&video.codec);

        is_4k && is_hevc
    }

    /// Check if a file qualifies for Profile B (universal playback).
    ///
    /// Profile B criteria:
    /// - Container is MP4 AND
    /// - Video codec is H.264/AVC AND
    /// - Resolution ≤1080p AND
    /// - No HDR (SDR only) AND
    /// - Has AAC audio track
    fn qualifies_for_profile_b(&self, info: &MediaInfo) -> bool {
        // Check container
        if !self.is_mp4_container(&info.container) {
            return false;
        }

        // Check video track
        let video = match info.video_tracks.first() {
            Some(v) => v,
            None => return false,
        };

        // Must be H.264
        if !self.is_h264_codec(&video.codec) {
            return false;
        }

        // Must be ≤1080p
        if video.width > 1920 || video.height > 1080 {
            return false;
        }

        // Must be SDR (no HDR)
        if let Some(hdr_format) = &video.hdr_format {
            match hdr_format {
                crate::probe::HdrFormat::Sdr => {
                    // OK, SDR
                }
                _ => return false, // Any HDR format disqualifies
            }
        }

        // Must not have Dolby Vision
        if video.dolby_vision.is_some() {
            return false;
        }

        // Check audio track - must have at least one AAC track
        let has_aac = info
            .audio_tracks
            .iter()
            .any(|a| self.is_aac_codec(&a.codec));

        has_aac
    }

    /// Determine if a file can be converted to Profile A.
    ///
    /// A file can be Profile A if it has HDR/DV content that can be properly processed.
    /// This means it either:
    /// - Already has HDR/DV metadata (can preserve it)
    /// - Has sufficient bit depth (10-bit+) that could theoretically support HDR
    fn can_be_profile_a(&self, info: &MediaInfo) -> bool {
        let video = match info.video_tracks.first() {
            Some(v) => v,
            None => return false,
        };

        // If it already has HDR/DV, it can definitely be Profile A
        if let Some(hdr_format) = &video.hdr_format {
            if !matches!(hdr_format, crate::probe::HdrFormat::Sdr) {
                return true;
            }
        }

        if video.dolby_vision.is_some() {
            return true;
        }

        // If it has 10-bit or higher depth, it could potentially support HDR
        // (though this would require tone-mapping or HDR grading, which is complex)
        // For now, we'll be conservative and only mark existing HDR content as
        // Profile A eligible
        if let Some(bit_depth) = video.bit_depth {
            if bit_depth >= 10 {
                // Has the technical capability, but without HDR metadata,
                // it's not really Profile A content
                // Return false for now - only actual HDR content qualifies
                return false;
            }
        }

        false
    }

    /// Determine if a file can be converted to Profile B.
    ///
    /// Almost any media file can be transcoded to Profile B format
    /// (MP4, H.264, ≤1080p, SDR, AAC stereo).
    ///
    /// We return false only for files that are missing essential components.
    fn can_be_profile_b(&self, info: &MediaInfo) -> bool {
        // Need at least one video track
        if info.video_tracks.is_empty() {
            return false;
        }

        // Need at least one audio track
        if info.audio_tracks.is_empty() {
            return false;
        }

        // If we have video and audio, we can transcode to Profile B
        true
    }

    /// Check if a codec string represents HEVC/H.265.
    fn is_hevc_codec(&self, codec: &str) -> bool {
        let codec_lower = codec.to_lowercase();
        codec_lower.contains("hevc")
            || codec_lower.contains("h265")
            || codec_lower.contains("h.265")
    }

    /// Check if a codec string represents H.264/AVC.
    fn is_h264_codec(&self, codec: &str) -> bool {
        let codec_lower = codec.to_lowercase();
        codec_lower.contains("h264") || codec_lower.contains("avc") || codec_lower.contains("h.264")
    }

    /// Check if a codec string represents AAC audio.
    fn is_aac_codec(&self, codec: &str) -> bool {
        let codec_lower = codec.to_lowercase();
        codec_lower.contains("aac")
    }

    /// Check if a container string represents MP4.
    fn is_mp4_container(&self, container: &str) -> bool {
        let container_lower = container.to_lowercase();
        container_lower.contains("mp4")
            || container_lower.contains("mpeg-4")
            || container_lower.contains("m4v")
    }
}

impl Default for ProfileClassifier {
    fn default() -> Self {
        Self::new()
    }
}
