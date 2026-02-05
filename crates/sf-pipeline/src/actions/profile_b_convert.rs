//! Profile B conversion action.

use async_trait::async_trait;

use crate::action::{Action, ActionResult};
use crate::context::ActionContext;

/// Convert the input to Profile B (H.264 High / AAC-LC stereo MP4).
#[derive(Debug)]
pub struct ProfileBConvertAction {
    crf: Option<u32>,
    preset: Option<String>,
}

impl ProfileBConvertAction {
    pub fn new(crf: Option<u32>, preset: Option<String>) -> Self {
        Self { crf, preset }
    }
}

#[async_trait]
impl Action for ProfileBConvertAction {
    fn name(&self) -> &'static str {
        "Profile B Convert"
    }

    async fn validate(&self, ctx: &ActionContext) -> sf_core::Result<()> {
        ctx.tools.require("ffmpeg")?;
        Ok(())
    }

    async fn execute(&self, ctx: &ActionContext) -> sf_core::Result<ActionResult> {
        if ctx.dry_run {
            tracing::info!("[DRY RUN] Would convert to Profile B");
            return Ok(ActionResult {
                output: None,
                summary: "Would convert to Profile B (H.264/AAC)".to_string(),
            });
        }

        let input = ctx.workspace.input();
        let output = ctx.workspace.output();

        // Build a config override if crf/preset were specified.
        let mut config = sf_core::config::ConversionConfig::default();
        if let Some(crf) = self.crf {
            config.video_crf = crf;
            config.adaptive_crf = false;
        }
        if let Some(ref preset) = self.preset {
            config.video_preset = preset.clone();
        }

        let height = ctx.media_info.primary_video().map(|v| v.height);

        sf_av::convert_to_profile_b(
            &ctx.tools,
            &input,
            &output,
            height,
            &config,
        )
        .await?;

        Ok(ActionResult {
            output: Some(output),
            summary: "Converted to Profile B (H.264/AAC)".to_string(),
        })
    }

    fn weight(&self) -> f32 {
        10.0
    }
}
