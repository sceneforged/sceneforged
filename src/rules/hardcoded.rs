//! Hardcoded rule definitions for profile-based media processing.
//!
//! This module defines static rules that determine what processing actions
//! to take based on media file profile classifications and characteristics.
//!
//! # Overview
//!
//! The hardcoded rules system replaces the previous TOML-based configuration
//! with a fixed set of processing rules that are applied based on the media
//! file's profile classification:
//!
//! - **Profile A**: High-quality source (HDR/DV/4K HEVC)
//! - **Profile B**: Universal playback (MP4, H.264, ≤1080p, SDR, AAC)
//! - **Profile C**: Unsupported/Pending (needs conversion)
//!
//! # Rules
//!
//! 1. **DV Profile 7 → 8**: Convert Dolby Vision Profile 7 to Profile 8 for compatibility
//! 2. **Profile A → B**: Generate Profile B version from Profile A source
//! 3. **Profile C HDR → A+B**: Process HDR content to both Profile A and B
//! 4. **Profile C SDR → B**: Convert SDR content directly to Profile B
//!
//! # Example
//!
//! ```no_run
//! use sceneforged::rules::get_applicable_rules;
//! use sceneforged::probe::probe_file;
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let path = Path::new("/path/to/video.mkv");
//! let media_info = probe_file(path)?;
//!
//! // Get rules that apply to this file
//! let rules = get_applicable_rules(&media_info);
//!
//! for rule in rules {
//!     println!("Rule: {} - {}", rule.name, rule.description);
//!     println!("Actions: {} action(s)", rule.actions.len());
//! }
//! # Ok(())
//! # }
//! ```

use crate::config::Action;
use crate::probe::MediaInfo;
use crate::scanner::ProfileClassifier;
use sceneforged_common::Profile;

/// A hardcoded processing rule.
#[derive(Debug, Clone)]
pub struct HardcodedRule {
    /// Unique identifier for the rule.
    pub id: &'static str,
    /// Human-readable name of the rule.
    pub name: &'static str,
    /// Description of what this rule does.
    pub description: &'static str,
    /// Profile(s) this rule creates/targets.
    pub target_profiles: Vec<Profile>,
    /// Actions to execute when this rule applies.
    pub actions: Vec<Action>,
}

/// Get all hardcoded rules that apply to the given media file.
///
/// This evaluates the media file's profile classification and characteristics
/// to determine which processing rules should be applied.
pub fn get_applicable_rules(info: &MediaInfo) -> Vec<HardcodedRule> {
    let classifier = ProfileClassifier::new();
    let classification = classifier.classify(info);
    let all_rules = create_rules();

    let mut applicable_rules = Vec::new();

    // Check for Dolby Vision Profile 7 (always highest priority)
    if has_dv_profile_7(info) {
        applicable_rules.push(all_rules[0].clone()); // DV Profile 7 → 8 conversion
    }

    // Apply rules based on profile classification
    match classification.profile {
        Profile::A => {
            // Profile A: Check if we need to generate Profile B
            // (This would require checking if Profile B already exists in the library,
            // which is beyond the scope of this simple rule module. For now, we'll
            // always include this rule and let the pipeline/processor decide if it's needed.)
            applicable_rules.push(all_rules[1].clone()); // Profile A → Generate Profile B
        }
        Profile::B => {
            // Profile B: Already in universal format, no action needed
        }
        Profile::C => {
            // Profile C: Needs conversion
            if classification.can_be_profile_a {
                // Has HDR/DV content, convert to both A and B
                applicable_rules.push(all_rules[2].clone()); // Profile C HDR → A + B
            } else {
                // No HDR content, convert to B only
                applicable_rules.push(all_rules[3].clone()); // Profile C SDR → B
            }
        }
    }

    applicable_rules
}

/// Check if the media file has Dolby Vision Profile 7.
fn has_dv_profile_7(info: &MediaInfo) -> bool {
    info.video_tracks
        .iter()
        .any(|track| match &track.dolby_vision {
            Some(dv) => dv.profile == 7,
            None => false,
        })
}

/// Create all hardcoded rules.
///
/// Rules are defined in priority order:
/// 1. DV Profile 7 → 8 conversion (highest priority, fixes compatibility)
/// 2. Profile A → Generate Profile B (create universal version)
/// 3. Profile C HDR → Profile A + B (process HDR content)
/// 4. Profile C SDR → Profile B (convert to universal format)
fn create_rules() -> Vec<HardcodedRule> {
    vec![
        // Rule 1: DV Profile 7 → Profile 8 Conversion
        HardcodedRule {
            id: "dv_p7_to_p8",
            name: "DV Profile 7 → Profile 8 Conversion",
            description: "Convert Dolby Vision Profile 7 (FEL/MEL) to Profile 8 for better compatibility. \
                          This preserves HDR quality while ensuring playback on more devices.",
            target_profiles: vec![Profile::A],
            actions: vec![Action::DvConvert { target_profile: 8 }],
        },
        // Rule 2: Profile A → Generate Profile B
        HardcodedRule {
            id: "profile_a_to_b",
            name: "Profile A → Generate Profile B",
            description: "Transcode Profile A (high-quality source) to Profile B (universal playback). \
                          Creates H.264/AAC/MP4 ≤1080p SDR version for maximum compatibility.",
            target_profiles: vec![Profile::B],
            actions: vec![
                // TODO: This needs to be replaced with actual transcode action
                // For now, we'll use Remux as a placeholder
                Action::Remux {
                    container: "mp4".to_string(),
                    keep_original: true,
                },
            ],
        },
        // Rule 3: Profile C HDR → Profile A + B
        HardcodedRule {
            id: "profile_c_hdr_to_a_and_b",
            name: "Profile C HDR → Profile A + B",
            description: "Process Profile C files with HDR/DV content. First, properly process HDR metadata \
                          to create Profile A, then generate Profile B for universal playback.",
            target_profiles: vec![Profile::A, Profile::B],
            actions: vec![
                // Step 1: Remux to MKV (Profile A format)
                Action::Remux {
                    container: "mkv".to_string(),
                    keep_original: false,
                },
                // Step 2: TODO: Generate Profile B from the new Profile A
                // This requires transcoding which needs to be implemented
            ],
        },
        // Rule 4: Profile C SDR → Profile B
        HardcodedRule {
            id: "profile_c_sdr_to_b",
            name: "Profile C SDR → Profile B",
            description: "Transcode Profile C files without HDR to Profile B (universal format). \
                          Creates H.264/AAC/MP4 ≤1080p SDR version for maximum compatibility.",
            target_profiles: vec![Profile::B],
            actions: vec![
                // TODO: This needs to be replaced with actual transcode action
                // For now, we'll use Remux as a placeholder
                Action::Remux {
                    container: "mp4".to_string(),
                    keep_original: false,
                },
            ],
        },
    ]
}

/// Get a hardcoded rule by its ID.
pub fn get_rule_by_id(id: &str) -> Option<HardcodedRule> {
    create_rules().into_iter().find(|rule| rule.id == id)
}

/// Get all hardcoded rules.
pub fn get_all_rules() -> Vec<HardcodedRule> {
    create_rules()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::{DolbyVisionInfo, HdrFormat, VideoTrack};
    use std::path::PathBuf;

    fn make_test_info(profile: u8) -> MediaInfo {
        MediaInfo {
            file_path: PathBuf::from("/test/video.mkv"),
            file_size: 1024 * 1024 * 1024,
            container: "Matroska".to_string(),
            duration: None,
            video_tracks: vec![VideoTrack {
                index: 0,
                codec: "HEVC".to_string(),
                width: 3840,
                height: 2160,
                frame_rate: Some(23.976),
                bit_depth: Some(10),
                hdr_format: Some(HdrFormat::DolbyVision),
                dolby_vision: Some(DolbyVisionInfo {
                    profile,
                    level: Some(6),
                    rpu_present: true,
                    el_present: true,
                    bl_present: true,
                    bl_compatibility_id: Some(1),
                }),
            }],
            audio_tracks: vec![],
            subtitle_tracks: vec![],
        }
    }

    #[test]
    fn test_dv_profile_7_detection() {
        let info = make_test_info(7);
        assert!(has_dv_profile_7(&info));

        let info = make_test_info(8);
        assert!(!has_dv_profile_7(&info));
    }

    #[test]
    fn test_get_applicable_rules_dv_p7() {
        let info = make_test_info(7);
        let rules = get_applicable_rules(&info);

        // Should include DV P7→P8 rule
        assert!(!rules.is_empty());
        assert_eq!(rules[0].id, "dv_p7_to_p8");
    }

    #[test]
    fn test_get_rule_by_id() {
        let rule = get_rule_by_id("dv_p7_to_p8");
        assert!(rule.is_some());
        assert_eq!(rule.unwrap().name, "DV Profile 7 → Profile 8 Conversion");

        let rule = get_rule_by_id("nonexistent");
        assert!(rule.is_none());
    }

    #[test]
    fn test_get_all_rules() {
        let rules = get_all_rules();
        assert_eq!(rules.len(), 4);
    }
}
