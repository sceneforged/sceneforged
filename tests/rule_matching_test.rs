//! Rule engine integration tests.
//!
//! Tests the rule matching engine with various expression trees and media
//! info scenarios, verifying priority ordering, disabled-rule skipping, and
//! complex boolean logic (AND, OR, NOT).

mod common;

use sf_core::{AudioCodec, Container, HdrFormat, RuleId, VideoCodec};
use sf_probe::{AudioTrack, DvInfo, MediaInfo, VideoTrack};
use sf_rules::{ActionConfig, Condition, Expr, Rule, RuleEngine};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Build a realistic media info for a 4K HDR DV movie.
fn make_dv_movie() -> MediaInfo {
    MediaInfo {
        file_path: PathBuf::from("/movies/Inception.mkv"),
        file_size: 50_000_000_000,
        container: Container::Mkv,
        duration: None,
        video_tracks: vec![VideoTrack {
            codec: VideoCodec::H265,
            width: 3840,
            height: 2160,
            frame_rate: Some(23.976),
            bit_depth: Some(10),
            hdr_format: HdrFormat::DolbyVision,
            dolby_vision: Some(DvInfo {
                profile: 7,
                rpu_present: true,
                el_present: true,
                bl_present: true,
            }),
            default: true,
            language: Some("eng".into()),
        }],
        audio_tracks: vec![AudioTrack {
            codec: AudioCodec::TrueHd,
            channels: 8,
            sample_rate: Some(48000),
            language: Some("eng".into()),
            atmos: true,
            default: true,
        }],
        subtitle_tracks: vec![],
    }
}

/// Build a simple SDR 1080p MP4 movie.
fn make_sdr_movie() -> MediaInfo {
    MediaInfo {
        file_path: PathBuf::from("/movies/simple.mp4"),
        file_size: 5_000_000_000,
        container: Container::Mp4,
        duration: None,
        video_tracks: vec![VideoTrack {
            codec: VideoCodec::H264,
            width: 1920,
            height: 1080,
            frame_rate: Some(24.0),
            bit_depth: Some(8),
            hdr_format: HdrFormat::Sdr,
            dolby_vision: None,
            default: true,
            language: Some("eng".into()),
        }],
        audio_tracks: vec![AudioTrack {
            codec: AudioCodec::Aac,
            channels: 2,
            sample_rate: Some(48000),
            language: Some("eng".into()),
            atmos: false,
            default: true,
        }],
        subtitle_tracks: vec![],
    }
}

fn make_rule(name: &str, priority: i32, enabled: bool, expr: Expr) -> Rule {
    Rule {
        id: RuleId::new(),
        name: name.into(),
        enabled,
        priority,
        expr,
        actions: vec![],
    }
}

// ---------------------------------------------------------------------------
// Simple conditions match
// ---------------------------------------------------------------------------

#[test]
fn simple_condition_codec_matches() {
    let info = make_dv_movie();
    let rule = make_rule(
        "h265_rule",
        10,
        true,
        Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
    );
    let engine = RuleEngine::new(vec![rule]);
    let matched = engine.find_matching_rule(&info);
    assert!(matched.is_some());
    assert_eq!(matched.unwrap().name, "h265_rule");
}

#[test]
fn simple_condition_container_matches() {
    let info = make_dv_movie();
    let rule = make_rule(
        "mkv_rule",
        10,
        true,
        Expr::Condition(Condition::Container(vec![Container::Mkv])),
    );
    let engine = RuleEngine::new(vec![rule]);
    assert!(engine.find_matching_rule(&info).is_some());
}

#[test]
fn simple_condition_does_not_match() {
    let info = make_dv_movie();
    let rule = make_rule(
        "mp4_rule",
        10,
        true,
        Expr::Condition(Condition::Container(vec![Container::Mp4])),
    );
    let engine = RuleEngine::new(vec![rule]);
    assert!(engine.find_matching_rule(&info).is_none());
}

#[test]
fn simple_condition_hdr_format() {
    let info = make_dv_movie();
    let rule = make_rule(
        "dv_rule",
        10,
        true,
        Expr::Condition(Condition::HdrFormat(vec![HdrFormat::DolbyVision])),
    );
    let engine = RuleEngine::new(vec![rule]);
    assert!(engine.find_matching_rule(&info).is_some());
}

#[test]
fn simple_condition_dv_profile() {
    let info = make_dv_movie();
    let rule = make_rule(
        "dv7_rule",
        10,
        true,
        Expr::Condition(Condition::DolbyVisionProfile(vec![7])),
    );
    let engine = RuleEngine::new(vec![rule]);
    assert!(engine.find_matching_rule(&info).is_some());

    let rule_p8 = make_rule(
        "dv8_rule",
        10,
        true,
        Expr::Condition(Condition::DolbyVisionProfile(vec![8])),
    );
    let engine = RuleEngine::new(vec![rule_p8]);
    assert!(engine.find_matching_rule(&info).is_none());
}

#[test]
fn simple_condition_audio_codec() {
    let info = make_dv_movie();
    let rule = make_rule(
        "truehd_rule",
        10,
        true,
        Expr::Condition(Condition::AudioCodec(vec![AudioCodec::TrueHd])),
    );
    let engine = RuleEngine::new(vec![rule]);
    assert!(engine.find_matching_rule(&info).is_some());
}

#[test]
fn simple_condition_has_atmos() {
    let info = make_dv_movie();
    let rule = make_rule(
        "atmos_rule",
        10,
        true,
        Expr::Condition(Condition::HasAtmos(true)),
    );
    let engine = RuleEngine::new(vec![rule]);
    assert!(engine.find_matching_rule(&info).is_some());

    let sdr = make_sdr_movie();
    assert!(engine.find_matching_rule(&sdr).is_none());
}

#[test]
fn simple_condition_min_resolution() {
    let info = make_dv_movie();
    let rule = make_rule(
        "4k_rule",
        10,
        true,
        Expr::Condition(Condition::MinResolution {
            width: 3840,
            height: 2160,
        }),
    );
    let engine = RuleEngine::new(vec![rule]);
    assert!(engine.find_matching_rule(&info).is_some());

    let sdr = make_sdr_movie();
    assert!(engine.find_matching_rule(&sdr).is_none());
}

#[test]
fn simple_condition_file_extension() {
    let info = make_dv_movie();
    let rule = make_rule(
        "mkv_ext_rule",
        10,
        true,
        Expr::Condition(Condition::FileExtension(vec!["mkv".into()])),
    );
    let engine = RuleEngine::new(vec![rule]);
    assert!(engine.find_matching_rule(&info).is_some());
}

// ---------------------------------------------------------------------------
// OR conditions match
// ---------------------------------------------------------------------------

#[test]
fn or_conditions_match_first() {
    let info = make_dv_movie();
    let rule = make_rule(
        "or_rule",
        10,
        true,
        Expr::Or(vec![
            Expr::Condition(Condition::Container(vec![Container::Mkv])),
            Expr::Condition(Condition::Container(vec![Container::Mp4])),
        ]),
    );
    let engine = RuleEngine::new(vec![rule]);
    assert!(engine.find_matching_rule(&info).is_some());
}

#[test]
fn or_conditions_match_second() {
    let info = make_sdr_movie();
    let rule = make_rule(
        "or_rule",
        10,
        true,
        Expr::Or(vec![
            Expr::Condition(Condition::Container(vec![Container::Mkv])),
            Expr::Condition(Condition::Container(vec![Container::Mp4])),
        ]),
    );
    let engine = RuleEngine::new(vec![rule]);
    assert!(engine.find_matching_rule(&info).is_some());
}

#[test]
fn or_conditions_none_match() {
    let info = make_dv_movie();
    let rule = make_rule(
        "or_rule",
        10,
        true,
        Expr::Or(vec![
            Expr::Condition(Condition::Codec(vec![VideoCodec::H264])),
            Expr::Condition(Condition::Codec(vec![VideoCodec::Av1])),
        ]),
    );
    let engine = RuleEngine::new(vec![rule]);
    assert!(engine.find_matching_rule(&info).is_none());
}

#[test]
fn complex_or_with_and() {
    let info = make_dv_movie();
    // (H265 AND MKV) OR (H264 AND MP4) -- first branch matches.
    let rule = make_rule(
        "complex_or",
        10,
        true,
        Expr::Or(vec![
            Expr::And(vec![
                Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
                Expr::Condition(Condition::Container(vec![Container::Mkv])),
            ]),
            Expr::And(vec![
                Expr::Condition(Condition::Codec(vec![VideoCodec::H264])),
                Expr::Condition(Condition::Container(vec![Container::Mp4])),
            ]),
        ]),
    );
    let engine = RuleEngine::new(vec![rule]);
    assert!(engine.find_matching_rule(&info).is_some());
}

// ---------------------------------------------------------------------------
// NOT conditions exclude
// ---------------------------------------------------------------------------

#[test]
fn not_condition_excludes() {
    let info = make_dv_movie();
    // NOT H264 -- should match since the movie is H265.
    let rule = make_rule(
        "not_h264",
        10,
        true,
        Expr::Not(Box::new(Expr::Condition(Condition::Codec(vec![
            VideoCodec::H264,
        ])))),
    );
    let engine = RuleEngine::new(vec![rule]);
    assert!(engine.find_matching_rule(&info).is_some());
}

#[test]
fn not_condition_does_not_exclude_when_true() {
    let info = make_dv_movie();
    // NOT H265 -- should NOT match since the movie IS H265.
    let rule = make_rule(
        "not_h265",
        10,
        true,
        Expr::Not(Box::new(Expr::Condition(Condition::Codec(vec![
            VideoCodec::H265,
        ])))),
    );
    let engine = RuleEngine::new(vec![rule]);
    assert!(engine.find_matching_rule(&info).is_none());
}

#[test]
fn and_with_not() {
    let info = make_dv_movie();
    // H265 AND NOT MP4 -- should match (it is H265 and NOT MP4).
    let rule = make_rule(
        "h265_not_mp4",
        10,
        true,
        Expr::And(vec![
            Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
            Expr::Not(Box::new(Expr::Condition(Condition::Container(vec![
                Container::Mp4,
            ])))),
        ]),
    );
    let engine = RuleEngine::new(vec![rule]);
    assert!(engine.find_matching_rule(&info).is_some());
}

// ---------------------------------------------------------------------------
// Priority ordering
// ---------------------------------------------------------------------------

#[test]
fn higher_priority_matched_first() {
    let info = make_dv_movie();

    let low = make_rule(
        "low",
        10,
        true,
        Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
    );
    let high = make_rule(
        "high",
        100,
        true,
        Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
    );
    // Insert in wrong order; engine should sort.
    let engine = RuleEngine::new(vec![low, high]);
    let matched = engine.find_matching_rule(&info).unwrap();
    assert_eq!(matched.name, "high");
}

#[test]
fn priority_ordering_with_multiple_rules() {
    let info = make_dv_movie();

    let rules = vec![
        make_rule(
            "p50",
            50,
            true,
            Expr::Condition(Condition::Container(vec![Container::Mkv])),
        ),
        make_rule(
            "p100",
            100,
            true,
            Expr::Condition(Condition::DolbyVisionProfile(vec![7])),
        ),
        make_rule(
            "p10",
            10,
            true,
            Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
        ),
    ];

    let engine = RuleEngine::new(rules);
    let all = engine.evaluate_all(&info);
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].name, "p100");
    assert_eq!(all[1].name, "p50");
    assert_eq!(all[2].name, "p10");
}

// ---------------------------------------------------------------------------
// Disabled rules skipped
// ---------------------------------------------------------------------------

#[test]
fn disabled_rule_skipped() {
    let info = make_dv_movie();

    let disabled = make_rule(
        "disabled_high",
        200,
        false,
        Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
    );
    let enabled = make_rule(
        "enabled_low",
        10,
        true,
        Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
    );

    let engine = RuleEngine::new(vec![disabled, enabled]);
    let matched = engine.find_matching_rule(&info).unwrap();
    assert_eq!(matched.name, "enabled_low");
}

#[test]
fn all_disabled_returns_none() {
    let info = make_dv_movie();

    let rules = vec![
        make_rule(
            "d1",
            100,
            false,
            Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
        ),
        make_rule(
            "d2",
            50,
            false,
            Expr::Condition(Condition::Container(vec![Container::Mkv])),
        ),
    ];

    let engine = RuleEngine::new(rules);
    assert!(engine.find_matching_rule(&info).is_none());
    assert!(engine.evaluate_all(&info).is_empty());
}

#[test]
fn disabled_rules_excluded_from_evaluate_all() {
    let info = make_dv_movie();

    let rules = vec![
        make_rule(
            "enabled",
            100,
            true,
            Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
        ),
        make_rule(
            "disabled",
            200,
            false,
            Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
        ),
    ];

    let engine = RuleEngine::new(rules);
    let matches = engine.evaluate_all(&info);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].name, "enabled");
}

// ---------------------------------------------------------------------------
// Rule engine with action configs
// ---------------------------------------------------------------------------

#[test]
fn rule_with_actions() {
    let info = make_dv_movie();

    let rule = Rule {
        id: RuleId::new(),
        name: "dv_convert".into(),
        enabled: true,
        priority: 100,
        expr: Expr::Condition(Condition::DolbyVisionProfile(vec![7])),
        actions: vec![
            ActionConfig::DvConvert { target_profile: 8 },
            ActionConfig::Remux {
                container: Container::Mp4,
                keep_original: false,
            },
        ],
    };

    let engine = RuleEngine::new(vec![rule]);
    let matched = engine.find_matching_rule(&info).unwrap();
    assert_eq!(matched.actions.len(), 2);
}

// ---------------------------------------------------------------------------
// Rule serialization round-trip through the engine
// ---------------------------------------------------------------------------

#[test]
fn rules_serialize_roundtrip() {
    let rules = vec![
        Rule {
            id: RuleId::new(),
            name: "complex".into(),
            enabled: true,
            priority: 50,
            expr: Expr::And(vec![
                Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
                Expr::Or(vec![
                    Expr::Condition(Condition::Container(vec![Container::Mkv])),
                    Expr::Not(Box::new(Expr::Condition(Condition::HasAtmos(false)))),
                ]),
            ]),
            actions: vec![ActionConfig::DvConvert { target_profile: 8 }],
        },
        Rule {
            id: RuleId::new(),
            name: "simple".into(),
            enabled: false,
            priority: 10,
            expr: Expr::Condition(Condition::Container(vec![Container::Mp4])),
            actions: vec![],
        },
    ];

    let json = sf_rules::serialize_rules(&rules).unwrap();
    let deserialized = sf_rules::deserialize_rules(&json).unwrap();
    assert_eq!(deserialized.len(), 2);
    assert_eq!(deserialized[0].name, "complex");
    assert_eq!(deserialized[1].name, "simple");
    assert!(!deserialized[1].enabled);

    // Verify the deserialized rules work the same in the engine.
    let info = make_dv_movie();
    let engine = RuleEngine::new(deserialized);
    let matched = engine.find_matching_rule(&info).unwrap();
    assert_eq!(matched.name, "complex");
}

// ---------------------------------------------------------------------------
// Edge case: empty expression trees
// ---------------------------------------------------------------------------

#[test]
fn empty_and_matches_everything() {
    let info = make_dv_movie();
    let rule = make_rule("empty_and", 10, true, Expr::And(vec![]));
    let engine = RuleEngine::new(vec![rule]);
    // Empty AND is vacuously true.
    assert!(engine.find_matching_rule(&info).is_some());
}

#[test]
fn empty_or_matches_nothing() {
    let info = make_dv_movie();
    let rule = make_rule("empty_or", 10, true, Expr::Or(vec![]));
    let engine = RuleEngine::new(vec![rule]);
    // Empty OR is vacuously false.
    assert!(engine.find_matching_rule(&info).is_none());
}
