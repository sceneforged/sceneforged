//! # sf-av
//!
//! Audio/video processing, probing, and external tool management for the
//! sceneforged pipeline.
//!
//! This crate provides:
//!
//! - **Tool discovery** ([`ToolRegistry`]) -- find and cache paths to ffmpeg,
//!   ffprobe, mediainfo, mkvmerge, mkvextract, and dovi_tool.
//! - **Command execution** ([`ToolCommand`]) -- async builder with timeout
//!   support for running external processes.
//! - **Workspace management** ([`Workspace`]) -- temporary directory lifecycle
//!   with safe finalization.
//! - **Probe backends** ([`probe::FfprobeProber`], [`probe::MediaInfoProber`])
//!   -- implement [`sf_probe::Prober`] by shelling out to CLI tools.
//! - **Action functions** ([`actions`]) -- remux, DV profile conversion,
//!   audio track addition, track stripping, and arbitrary command execution.

pub mod actions;
pub mod command;
pub mod probe;
pub mod tools;
pub mod workspace;

// ---- Re-exports for convenience ----

pub use command::{ToolCommand, ToolOutput};
pub use probe::{FfprobeProber, MediaInfoProber};
pub use tools::{ToolConfig, ToolInfo, ToolRegistry};
pub use workspace::Workspace;

// Action functions
pub use actions::{
    add_compat_audio, adaptive_crf, convert_dv_profile, convert_to_profile_b,
    convert_to_profile_b_with_progress, exec_command, remux, strip_tracks,
};
