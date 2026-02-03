//! sf-core: shared types, IDs, errors, configuration, and event system.
//!
//! This crate is the foundational dependency for all other sf-* crates,
//! providing type-safe identifiers, a unified error type, media-domain
//! enums, application configuration, and a broadcast event bus.

pub mod config;
pub mod error;
pub mod events;
pub mod ids;
pub mod media;

// Re-export the most commonly used items at the crate root.
pub use error::{Error, Result};
pub use ids::*;
pub use media::*;
