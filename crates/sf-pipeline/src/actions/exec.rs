//! Arbitrary external command action.

use async_trait::async_trait;

use crate::action::{Action, ActionResult};
use crate::context::ActionContext;

/// Execute an external command with `{input}` / `{output}` substitution.
///
/// This is a general escape hatch for custom pipeline steps that are not
/// covered by the built-in actions.
#[derive(Debug)]
pub struct ExecAction {
    command: String,
    args: Vec<String>,
}

impl ExecAction {
    /// Create a new exec action.
    pub fn new(command: String, args: Vec<String>) -> Self {
        Self { command, args }
    }
}

#[async_trait]
impl Action for ExecAction {
    fn name(&self) -> &'static str {
        "Execute Command"
    }

    async fn validate(&self, _ctx: &ActionContext) -> sf_core::Result<()> {
        // We could check `which` for the command, but the user might use an
        // absolute path or something that is only available at runtime.
        Ok(())
    }

    async fn execute(&self, ctx: &ActionContext) -> sf_core::Result<ActionResult> {
        if ctx.dry_run {
            tracing::info!(
                "[DRY RUN] Would execute: {} {:?}",
                self.command,
                self.args,
            );
            return Ok(ActionResult {
                output: None,
                summary: format!("Would execute: {}", self.command),
            });
        }

        sf_av::exec_command(&ctx.workspace, &self.command, &self.args).await?;

        Ok(ActionResult {
            output: Some(ctx.workspace.output()),
            summary: format!("Executed: {}", self.command),
        })
    }
}
