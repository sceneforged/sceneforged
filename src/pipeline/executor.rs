use crate::config::Action;
use anyhow::{Context, Result};
use sceneforged_av::Workspace;
use std::path::Path;

/// Progress callback type
pub type ProgressCallback = Box<dyn Fn(f32, &str) + Send + Sync>;

/// Execute a pipeline of actions
pub struct PipelineExecutor {
    workspace: Workspace,
    dry_run: bool,
    progress_callback: Option<ProgressCallback>,
}

impl PipelineExecutor {
    pub fn new(input: &Path, dry_run: bool) -> Result<Self> {
        let workspace = Workspace::new(input)?;
        Ok(Self {
            workspace,
            dry_run,
            progress_callback: None,
        })
    }

    pub fn with_progress_callback(mut self, callback: ProgressCallback) -> Self {
        self.progress_callback = Some(callback);
        self
    }

    fn report_progress(&self, progress: f32, step: &str) {
        if let Some(ref cb) = self.progress_callback {
            cb(progress, step);
        }
        tracing::info!("[{:.0}%] {}", progress, step);
    }

    /// Execute a list of actions
    pub fn execute(self, actions: &[Action]) -> Result<std::path::PathBuf> {
        if actions.is_empty() {
            anyhow::bail!("No actions to execute");
        }

        let total_actions = actions.len();

        for (i, action) in actions.iter().enumerate() {
            let progress = (i as f32 / total_actions as f32) * 100.0;
            let step_name = action_name(action);
            self.report_progress(progress, &format!("Starting: {}", step_name));

            if self.dry_run {
                tracing::info!("[DRY RUN] Would execute: {:?}", action);
                continue;
            }

            self.execute_action(action)
                .with_context(|| format!("Failed to execute action: {}", step_name))?;
        }

        self.report_progress(100.0, "Finalizing");

        if self.dry_run {
            tracing::info!("[DRY RUN] Would finalize output");
            Ok(self.workspace.input().to_path_buf())
        } else {
            self.workspace
                .finalize(None)
                .map_err(|e| anyhow::anyhow!("{}", e))
        }
    }

    fn execute_action(&self, action: &Action) -> Result<()> {
        match action {
            Action::DvConvert { target_profile } => {
                use sceneforged_av::actions::{convert_dv_profile, DvProfile};
                let profile = match target_profile {
                    8 => DvProfile::Profile8,
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Unsupported DV profile: {}. Only profile 8 is currently supported.",
                            target_profile
                        ))
                    }
                };
                convert_dv_profile(self.workspace(), profile).map_err(|e| anyhow::anyhow!("{}", e))
            }
            Action::Remux {
                container,
                keep_original: _,
            } => {
                use sceneforged_av::actions::{remux, Container};
                let target: Container = container
                    .parse()
                    .map_err(|e: String| anyhow::anyhow!("{}", e))?;
                remux(self.workspace(), target).map_err(|e| anyhow::anyhow!("{}", e))
            }
            Action::AddCompatAudio {
                source_codec,
                target_codec,
            } => {
                use sceneforged_av::actions::{add_compat_audio, AudioCodec};
                let target = match target_codec.to_lowercase().as_str() {
                    "aac" => AudioCodec::Aac,
                    "ac3" | "ac-3" => AudioCodec::Ac3,
                    "eac3" | "e-ac-3" => AudioCodec::Eac3,
                    "flac" => AudioCodec::Flac,
                    "opus" => AudioCodec::Opus,
                    _ => return Err(anyhow::anyhow!("Unsupported audio codec: {}", target_codec)),
                };
                add_compat_audio(self.workspace(), source_codec, target)
                    .map_err(|e| anyhow::anyhow!("{}", e))
            }
            Action::StripTracks {
                track_types,
                languages,
            } => {
                use sceneforged_av::actions::{strip_tracks, StripConfig};
                let mut config = StripConfig::new();
                for track_type in track_types {
                    match track_type.to_lowercase().as_str() {
                        "audio" => config.strip_audio = true,
                        "subtitle" | "subtitles" => config.strip_subtitles = true,
                        _ => {}
                    }
                }
                if !languages.is_empty() {
                    config.strip_languages = languages.clone();
                }
                strip_tracks(self.workspace(), &config).map_err(|e| anyhow::anyhow!("{}", e))
            }
            Action::Exec { command, args } => {
                use super::actions::exec_command;
                exec_command(self.workspace(), command, args)
            }
        }
    }

    /// Get workspace for action implementations
    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }
}

fn action_name(action: &Action) -> &'static str {
    match action {
        Action::DvConvert { .. } => "Dolby Vision Convert",
        Action::Remux { .. } => "Remux",
        Action::AddCompatAudio { .. } => "Add Compatibility Audio",
        Action::StripTracks { .. } => "Strip Tracks",
        Action::Exec { .. } => "Execute Command",
    }
}
