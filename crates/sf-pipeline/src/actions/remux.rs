//! Container remux action.

use async_trait::async_trait;

use crate::action::{Action, ActionResult};
use crate::context::ActionContext;

/// Remux the input into a different container format.
#[derive(Debug)]
pub struct RemuxAction {
    container: sf_core::Container,
}

impl RemuxAction {
    /// Create a new remux action targeting the given container.
    pub fn new(container: sf_core::Container) -> Self {
        Self { container }
    }
}

#[async_trait]
impl Action for RemuxAction {
    fn name(&self) -> &'static str {
        "Remux"
    }

    async fn validate(&self, ctx: &ActionContext) -> sf_core::Result<()> {
        // remux needs at least ffmpeg; mkvmerge is preferred for MKV but optional.
        ctx.tools.require("ffmpeg")?;
        Ok(())
    }

    async fn execute(&self, ctx: &ActionContext) -> sf_core::Result<ActionResult> {
        if ctx.dry_run {
            tracing::info!("[DRY RUN] Would remux to {}", self.container);
            return Ok(ActionResult {
                output: None,
                summary: format!("Would remux to {}", self.container),
            });
        }

        sf_av::remux(&ctx.workspace, &ctx.tools, self.container).await?;

        Ok(ActionResult {
            output: Some(ctx.workspace.output()),
            summary: format!("Remuxed to {}", self.container),
        })
    }

    fn weight(&self) -> f32 {
        2.0
    }
}
