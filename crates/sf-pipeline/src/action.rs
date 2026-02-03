//! The [`Action`] trait defines a single pipeline step.
//!
//! Each action validates its preconditions, executes its work, and optionally
//! supports rollback.  Actions may declare themselves parallelizable so the
//! executor can run compatible actions concurrently within a stage.

use std::path::PathBuf;

use async_trait::async_trait;

use crate::context::ActionContext;

/// Result of a successfully executed action.
#[derive(Debug, Clone)]
pub struct ActionResult {
    /// Path to the output file produced by this action (if any).
    pub output: Option<PathBuf>,
    /// Human-readable summary of what the action did.
    pub summary: String,
}

/// A single step in a processing pipeline.
///
/// Implementors provide the core logic for one media transformation (remux,
/// DV convert, audio addition, etc.).
#[async_trait]
pub trait Action: Send + Sync {
    /// A short, human-readable name for this action (e.g. "Remux").
    fn name(&self) -> &'static str;

    /// Validate that all preconditions are met before execution.
    ///
    /// This is called once, before the executor begins running the pipeline.
    /// Implementations should check for required tools, compatible input, etc.
    async fn validate(&self, ctx: &ActionContext) -> sf_core::Result<()>;

    /// Perform the action.
    async fn execute(&self, ctx: &ActionContext) -> sf_core::Result<ActionResult>;

    /// Undo any side effects of a previously successful [`execute`](Action::execute).
    ///
    /// Called in reverse order when a later action fails.  The default
    /// implementation is a no-op.
    async fn rollback(&self, _ctx: &ActionContext) -> sf_core::Result<()> {
        Ok(())
    }

    /// Whether this action can run in parallel with other parallelizable
    /// actions within the same stage.
    ///
    /// Returns `false` by default.
    fn parallelizable(&self) -> bool {
        false
    }

    /// Relative weight of this action for progress reporting.
    ///
    /// The executor normalises weights across all actions so that heavier
    /// actions consume a proportionally larger share of the progress bar.
    /// Default is `1.0`.
    fn weight(&self) -> f32 {
        1.0
    }
}
