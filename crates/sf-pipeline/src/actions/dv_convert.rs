//! Dolby Vision profile conversion action.

use async_trait::async_trait;

use crate::action::{Action, ActionResult};
use crate::context::ActionContext;

/// Convert Dolby Vision to a target profile via `dovi_tool`.
#[derive(Debug)]
pub struct DvConvertAction {
    target_profile: u8,
}

impl DvConvertAction {
    /// Create a new DV conversion action.
    pub fn new(target_profile: u8) -> Self {
        Self { target_profile }
    }
}

#[async_trait]
impl Action for DvConvertAction {
    fn name(&self) -> &'static str {
        "Dolby Vision Convert"
    }

    async fn validate(&self, ctx: &ActionContext) -> sf_core::Result<()> {
        ctx.tools.require("ffmpeg")?;
        ctx.tools.require("dovi_tool")?;
        ctx.tools.require("mkvmerge")?;
        Ok(())
    }

    async fn execute(&self, ctx: &ActionContext) -> sf_core::Result<ActionResult> {
        if ctx.dry_run {
            tracing::info!(
                "[DRY RUN] Would convert DV to profile {}",
                self.target_profile
            );
            return Ok(ActionResult {
                output: None,
                summary: format!(
                    "Would convert DV to profile {}",
                    self.target_profile
                ),
            });
        }

        sf_av::convert_dv_profile(&ctx.workspace, &ctx.tools, self.target_profile).await?;

        Ok(ActionResult {
            output: Some(ctx.workspace.output()),
            summary: format!("Converted DV to profile {}", self.target_profile),
        })
    }

    fn weight(&self) -> f32 {
        3.0
    }
}
