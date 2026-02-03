//! Add compatibility audio track action.

use async_trait::async_trait;

use crate::action::{Action, ActionResult};
use crate::context::ActionContext;

/// Add a compatibility audio track by transcoding from an existing stream.
#[derive(Debug)]
pub struct AddCompatAudioAction {
    source_codec: sf_core::AudioCodec,
    target_codec: sf_core::AudioCodec,
}

impl AddCompatAudioAction {
    /// Create a new action that finds the first audio track matching
    /// `source_codec` and transcodes it to `target_codec`.
    pub fn new(source_codec: sf_core::AudioCodec, target_codec: sf_core::AudioCodec) -> Self {
        Self {
            source_codec,
            target_codec,
        }
    }
}

#[async_trait]
impl Action for AddCompatAudioAction {
    fn name(&self) -> &'static str {
        "Add Compatibility Audio"
    }

    async fn validate(&self, ctx: &ActionContext) -> sf_core::Result<()> {
        ctx.tools.require("ffmpeg")?;
        Ok(())
    }

    async fn execute(&self, ctx: &ActionContext) -> sf_core::Result<ActionResult> {
        if ctx.dry_run {
            tracing::info!(
                "[DRY RUN] Would add {} compat track from {}",
                self.target_codec,
                self.source_codec,
            );
            return Ok(ActionResult {
                output: None,
                summary: format!(
                    "Would add {} compat track from {}",
                    self.target_codec, self.source_codec,
                ),
            });
        }

        // Find the first audio track matching the source codec.
        let source_track = ctx
            .media_info
            .audio_tracks
            .iter()
            .position(|t| t.codec == self.source_codec)
            .unwrap_or(0);

        sf_av::add_compat_audio(&ctx.workspace, &ctx.tools, source_track, self.target_codec)
            .await?;

        Ok(ActionResult {
            output: Some(ctx.workspace.output()),
            summary: format!(
                "Added {} compat track from {} (track {})",
                self.target_codec, self.source_codec, source_track,
            ),
        })
    }

    fn weight(&self) -> f32 {
        2.0
    }
}
