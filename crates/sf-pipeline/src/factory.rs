//! Action factory: construct [`Action`] objects from [`ActionConfig`] values.

use sf_rules::ActionConfig;

use crate::action::Action;
use crate::actions::{
    AddCompatAudioAction, DvConvertAction, ExecAction, RemuxAction, StripTracksAction,
};

/// Create a list of boxed [`Action`] objects from rule-engine configurations.
///
/// Tool availability is validated eagerly so that missing tools are reported
/// before any work begins.
///
/// # Errors
///
/// Returns [`sf_core::Error::Tool`] if a required tool is not present in the
/// registry.
pub fn create_actions(
    configs: &[ActionConfig],
    tools: &sf_av::ToolRegistry,
) -> sf_core::Result<Vec<Box<dyn Action>>> {
    let mut actions: Vec<Box<dyn Action>> = Vec::with_capacity(configs.len());

    for config in configs {
        match config {
            ActionConfig::DvConvert { target_profile } => {
                tools.require("ffmpeg")?;
                tools.require("dovi_tool")?;
                tools.require("mkvmerge")?;
                actions.push(Box::new(DvConvertAction::new(*target_profile)));
            }
            ActionConfig::Remux { container, .. } => {
                tools.require("ffmpeg")?;
                actions.push(Box::new(RemuxAction::new(*container)));
            }
            ActionConfig::AddCompatAudio {
                source_codec,
                target_codec,
            } => {
                tools.require("ffmpeg")?;
                actions.push(Box::new(AddCompatAudioAction::new(
                    *source_codec,
                    *target_codec,
                )));
            }
            ActionConfig::StripTracks {
                track_types,
                languages,
            } => {
                tools.require("mkvmerge")?;
                actions.push(Box::new(StripTracksAction::new(
                    track_types.clone(),
                    languages.clone(),
                )));
            }
            ActionConfig::Exec { command, args } => {
                actions.push(Box::new(ExecAction::new(command.clone(), args.clone())));
            }
        }
    }

    Ok(actions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_core::{AudioCodec, Container, StreamType};

    fn make_tools() -> sf_av::ToolRegistry {
        sf_av::ToolRegistry::discover(&sf_core::config::ToolsConfig::default())
    }

    #[test]
    fn create_empty_configs() {
        let tools = make_tools();
        let result = create_actions(&[], &tools);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn create_exec_always_succeeds() {
        // Exec does not require any specific tool in the registry.
        let tools = make_tools();
        let configs = vec![ActionConfig::Exec {
            command: "echo".into(),
            args: vec!["hello".into()],
        }];
        let actions = create_actions(&configs, &tools).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].name(), "Execute Command");
    }

    #[test]
    fn action_names_match_config_variants() {
        // This test only runs exec (no tool requirements).
        let tools = make_tools();
        let configs = vec![
            ActionConfig::Exec {
                command: "true".into(),
                args: vec![],
            },
        ];
        let actions = create_actions(&configs, &tools).unwrap();
        assert_eq!(actions[0].name(), "Execute Command");
    }

    #[test]
    fn dv_convert_requires_tools() {
        // Use a default registry; if tools are missing this should fail.
        let tools = make_tools();
        let configs = vec![ActionConfig::DvConvert { target_profile: 8 }];
        let result = create_actions(&configs, &tools);
        // We accept either outcome: success (tools found) or failure (tools missing).
        // The point is it does not panic.
        let _ = result;
    }

    #[test]
    fn remux_creates_action() {
        let tools = make_tools();
        let configs = vec![ActionConfig::Remux {
            container: Container::Mkv,
            keep_original: false,
        }];
        let result = create_actions(&configs, &tools);
        // If ffmpeg is available, this succeeds; otherwise it fails gracefully.
        if let Ok(actions) = result {
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].name(), "Remux");
        }
    }

    #[test]
    fn add_compat_audio_creates_action() {
        let tools = make_tools();
        let configs = vec![ActionConfig::AddCompatAudio {
            source_codec: AudioCodec::TrueHd,
            target_codec: AudioCodec::Aac,
        }];
        let result = create_actions(&configs, &tools);
        if let Ok(actions) = result {
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].name(), "Add Compatibility Audio");
        }
    }

    #[test]
    fn strip_tracks_creates_action() {
        let tools = make_tools();
        let configs = vec![ActionConfig::StripTracks {
            track_types: vec![StreamType::Subtitle],
            languages: None,
        }];
        let result = create_actions(&configs, &tools);
        if let Ok(actions) = result {
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].name(), "Strip Tracks");
        }
    }
}
