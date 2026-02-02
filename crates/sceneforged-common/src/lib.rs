//! Sceneforged-Common: Shared types, constants, and utilities.
//!
//! This crate provides common functionality used across sceneforged:
//!
//! - **Typed IDs**: Type-safe UUID wrappers for users, items, libraries, etc.
//! - **Core Types**: Enums for media types, item kinds, image types, and streams
//! - **Path Utilities**: Functions to detect file types by extension
//! - **Error Handling**: Common error types and result aliases
//!
//! # Examples
//!
//! ```
//! use sceneforged_common::{ItemId, MediaType, Error, Result};
//! use sceneforged_common::paths::is_video_file;
//! use std::path::Path;
//!
//! // Create typed IDs
//! let item_id = ItemId::new();
//!
//! // Work with media types
//! let media_type = MediaType::Movies;
//!
//! // Check file types
//! assert!(is_video_file(Path::new("movie.mkv")));
//!
//! // Use common error types
//! fn example() -> Result<()> {
//!     Err(Error::not_found("item"))
//! }
//! ```

pub mod error;
pub mod ids;
pub mod paths;
pub mod types;

pub use error::{Error, Result};
pub use ids::*;
pub use types::*;
