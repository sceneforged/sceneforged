mod matcher;

#[cfg(test)]
mod test_fixtures;

pub use matcher::*;

use crate::config::Rule;
use crate::probe::MediaInfo;

/// Find the first matching rule for the given media info
pub fn find_matching_rule<'a>(info: &MediaInfo, rules: &'a [Rule]) -> Option<&'a Rule> {
    // Rules are pre-sorted by priority at config load time
    rules.iter().find(|rule| matches_rule(info, rule))
}

/// Find all matching rules for the given media info
pub fn find_all_matching_rules<'a>(info: &MediaInfo, rules: &'a [Rule]) -> Vec<&'a Rule> {
    // Rules are pre-sorted by priority at config load time
    rules
        .iter()
        .filter(|rule| matches_rule(info, rule))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Action, MatchConditions};
    use crate::rules::test_fixtures::make_dv_p7_file;

    fn make_test_rules() -> Vec<Rule> {
        vec![
            Rule {
                name: "dv_p7_convert".to_string(),
                enabled: true,
                priority: 100,
                match_conditions: MatchConditions {
                    dolby_vision_profiles: vec![7],
                    ..Default::default()
                },
                actions: vec![Action::DvConvert { target_profile: 8 }],
                normalized: None,
            },
            Rule {
                name: "avi_remux".to_string(),
                enabled: true,
                priority: 50,
                match_conditions: MatchConditions {
                    containers: vec!["avi".to_string()],
                    ..Default::default()
                },
                actions: vec![Action::Remux {
                    container: "mkv".to_string(),
                    keep_original: false,
                }],
                normalized: None,
            },
            Rule {
                name: "disabled_rule".to_string(),
                enabled: false,
                priority: 200,
                match_conditions: MatchConditions::default(),
                actions: vec![],
                normalized: None,
            },
        ]
    }

    #[test]
    fn test_find_matching_rule() {
        let rules = make_test_rules();
        let info = make_dv_p7_file();

        let matched = find_matching_rule(&info, &rules);
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().name, "dv_p7_convert");
    }
}
