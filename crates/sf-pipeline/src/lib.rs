//! # sf-pipeline
//!
//! Orchestration of encoding pipelines for media processing.
//!
//! This crate provides:
//!
//! - **[`Action`]** trait -- a single pipeline step with validate / execute /
//!   rollback semantics.
//! - **[`ActionContext`]** -- shared execution context (workspace, media info,
//!   tool registry, cancellation, progress).
//! - **Built-in actions** ([`actions`]) -- DV convert, remux, add compat audio,
//!   strip tracks, exec.
//! - **[`PipelineExecutor`]** -- groups actions into stages, runs them
//!   sequentially (with intra-stage parallelism), tracks progress, and rolls
//!   back on failure.
//! - **[`create_actions`]** -- factory function that builds action objects from
//!   rule-engine [`ActionConfig`](sf_rules::ActionConfig) values.

pub mod action;
pub mod actions;
pub mod context;
pub mod executor;
pub mod factory;

// Re-export key types at the crate root.
pub use action::{Action, ActionResult};
pub use context::{ActionContext, ProgressSender};
pub use executor::PipelineExecutor;
pub use factory::create_actions;
