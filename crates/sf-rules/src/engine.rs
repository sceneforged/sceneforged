//! The [`RuleEngine`] evaluates media files against a set of rules.

use sf_probe::MediaInfo;

use crate::expr;
use crate::rule::Rule;

/// Rule engine that holds sorted rules and evaluates media files against them.
#[derive(Debug, Clone)]
pub struct RuleEngine {
    /// Rules sorted by priority descending (highest priority first).
    rules: Vec<Rule>,
}

impl RuleEngine {
    /// Create a new rule engine, sorting rules by priority descending.
    pub fn new(mut rules: Vec<Rule>) -> Self {
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        Self { rules }
    }

    /// Return the first enabled rule whose expression matches the media info.
    pub fn find_matching_rule(&self, info: &MediaInfo) -> Option<&Rule> {
        self.rules
            .iter()
            .filter(|rule| rule.enabled)
            .find(|rule| expr::evaluate(&rule.expr, info))
    }

    /// Return all enabled rules whose expressions match the media info.
    pub fn evaluate_all(&self, info: &MediaInfo) -> Vec<&Rule> {
        self.rules
            .iter()
            .filter(|rule| rule.enabled)
            .filter(|rule| expr::evaluate(&rule.expr, info))
            .collect()
    }

    /// Return a reference to the internal rules slice.
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action_config::ActionConfig;
    use crate::condition::Condition;
    use crate::expr::Expr;
    use sf_core::{AudioCodec, Container, HdrFormat, RuleId, VideoCodec};
    use sf_probe::{AudioTrack, DvInfo, VideoTrack};
    use std::path::PathBuf;

    fn make_test_info() -> MediaInfo {
        MediaInfo {
            file_path: PathBuf::from("/test/movie.mkv"),
            file_size: 1024 * 1024 * 1024,
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
                language: Some("eng".to_string()),
            }],
            audio_tracks: vec![AudioTrack {
                codec: AudioCodec::TrueHd,
                channels: 8,
                sample_rate: Some(48000),
                language: Some("eng".to_string()),
                atmos: true,
                default: true,
            }],
            subtitle_tracks: vec![],
        }
    }

    fn make_test_rules() -> Vec<Rule> {
        vec![
            Rule {
                id: RuleId::new(),
                name: "dv_p7_convert".to_string(),
                enabled: true,
                priority: 100,
                expr: Expr::Condition(Condition::DolbyVisionProfile(vec![7])),
                actions: vec![ActionConfig::DvConvert { target_profile: 8 }],
            },
            Rule {
                id: RuleId::new(),
                name: "mkv_remux".to_string(),
                enabled: true,
                priority: 50,
                expr: Expr::Condition(Condition::Container(vec![Container::Mkv])),
                actions: vec![ActionConfig::Remux {
                    container: Container::Mp4,
                    keep_original: false,
                }],
            },
            Rule {
                id: RuleId::new(),
                name: "disabled_rule".to_string(),
                enabled: false,
                priority: 200,
                expr: Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
                actions: vec![],
            },
            Rule {
                id: RuleId::new(),
                name: "low_priority_match".to_string(),
                enabled: true,
                priority: 10,
                expr: Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
                actions: vec![],
            },
        ]
    }

    #[test]
    fn rules_sorted_by_priority_descending() {
        let engine = RuleEngine::new(make_test_rules());
        let priorities: Vec<i32> = engine.rules().iter().map(|r| r.priority).collect();
        assert_eq!(priorities, vec![200, 100, 50, 10]);
    }

    #[test]
    fn find_matching_rule_skips_disabled() {
        let info = make_test_info();
        let engine = RuleEngine::new(make_test_rules());

        // The highest priority rule (200) is disabled, so the next enabled
        // matching rule should be "dv_p7_convert" (priority 100).
        let matched = engine.find_matching_rule(&info);
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().name, "dv_p7_convert");
    }

    #[test]
    fn find_matching_rule_returns_first_by_priority() {
        let info = make_test_info();
        let rules = vec![
            Rule {
                id: RuleId::new(),
                name: "low_priority".to_string(),
                enabled: true,
                priority: 10,
                expr: Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
                actions: vec![],
            },
            Rule {
                id: RuleId::new(),
                name: "high_priority".to_string(),
                enabled: true,
                priority: 100,
                expr: Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
                actions: vec![],
            },
        ];
        let engine = RuleEngine::new(rules);
        let matched = engine.find_matching_rule(&info);
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().name, "high_priority");
    }

    #[test]
    fn find_matching_rule_returns_none_when_nothing_matches() {
        let info = make_test_info();
        let rules = vec![Rule {
            id: RuleId::new(),
            name: "mp4_only".to_string(),
            enabled: true,
            priority: 100,
            expr: Expr::Condition(Condition::Container(vec![Container::Mp4])),
            actions: vec![],
        }];
        let engine = RuleEngine::new(rules);
        assert!(engine.find_matching_rule(&info).is_none());
    }

    #[test]
    fn evaluate_all_returns_all_enabled_matches() {
        let info = make_test_info();
        let engine = RuleEngine::new(make_test_rules());

        let matches = engine.evaluate_all(&info);
        // Disabled rule is skipped. The other 3 enabled rules should all match:
        // - dv_p7_convert (DV profile 7 matches)
        // - mkv_remux (container MKV matches)
        // - low_priority_match (codec H265 matches)
        assert_eq!(matches.len(), 3);
        // Results should be in priority order (descending)
        assert_eq!(matches[0].name, "dv_p7_convert");
        assert_eq!(matches[1].name, "mkv_remux");
        assert_eq!(matches[2].name, "low_priority_match");
    }

    #[test]
    fn evaluate_all_skips_disabled_rules() {
        let info = make_test_info();
        let engine = RuleEngine::new(make_test_rules());

        let matches = engine.evaluate_all(&info);
        for rule in &matches {
            assert!(rule.enabled, "disabled rule should not appear in results");
            assert_ne!(rule.name, "disabled_rule");
        }
    }

    #[test]
    fn evaluate_all_returns_empty_when_nothing_matches() {
        let info = make_test_info();
        let rules = vec![Rule {
            id: RuleId::new(),
            name: "mp4_only".to_string(),
            enabled: true,
            priority: 100,
            expr: Expr::Condition(Condition::Container(vec![Container::Mp4])),
            actions: vec![],
        }];
        let engine = RuleEngine::new(rules);
        assert!(engine.evaluate_all(&info).is_empty());
    }

    #[test]
    fn complex_expr_matching() {
        let info = make_test_info();
        // Match: (H265 AND DV Profile 7) AND (HasAtmos(true) OR AudioCodec(AAC))
        let rule = Rule {
            id: RuleId::new(),
            name: "complex_rule".to_string(),
            enabled: true,
            priority: 50,
            expr: Expr::And(vec![
                Expr::And(vec![
                    Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
                    Expr::Condition(Condition::DolbyVisionProfile(vec![7])),
                ]),
                Expr::Or(vec![
                    Expr::Condition(Condition::HasAtmos(true)),
                    Expr::Condition(Condition::AudioCodec(vec![AudioCodec::Aac])),
                ]),
            ]),
            actions: vec![ActionConfig::DvConvert { target_profile: 8 }],
        };
        let engine = RuleEngine::new(vec![rule]);
        assert!(engine.find_matching_rule(&info).is_some());
    }

    #[test]
    fn all_disabled_returns_none() {
        let info = make_test_info();
        let rules = vec![
            Rule {
                id: RuleId::new(),
                name: "disabled_1".to_string(),
                enabled: false,
                priority: 100,
                expr: Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
                actions: vec![],
            },
            Rule {
                id: RuleId::new(),
                name: "disabled_2".to_string(),
                enabled: false,
                priority: 50,
                expr: Expr::Condition(Condition::Container(vec![Container::Mkv])),
                actions: vec![],
            },
        ];
        let engine = RuleEngine::new(rules);
        assert!(engine.find_matching_rule(&info).is_none());
        assert!(engine.evaluate_all(&info).is_empty());
    }
}
