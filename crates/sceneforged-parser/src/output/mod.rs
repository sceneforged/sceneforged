//! Output formatting for different consumers.
//!
//! This module provides trait-based output formatting for different media
//! management systems and platforms. Each formatter knows how to structure
//! parsed release data in a way that's compatible with its target system.
//!
//! # Available Formats
//!
//! - [`SonarrFormat`]: Compatible with Sonarr's expected format
//! - [`PlexFormat`]: Optimized for Plex naming conventions
//! - [`GenericFormat`]: Simple human-readable string representation
//!
//! # Example
//!
//! ```
//! use sceneforged_parser::{parse, output::{OutputFormat, SonarrFormat, PlexFormat}};
//!
//! let release = parse("The.Matrix.1999.1080p.BluRay.x264-GROUP");
//!
//! let sonarr = SonarrFormat;
//! let sonarr_output = sonarr.format(&release);
//!
//! let plex = PlexFormat;
//! let plex_output = plex.format(&release);
//! ```

mod generic;
mod plex;
mod sonarr;

pub use generic::{GenericFormat, GenericOutput};
pub use plex::{PlexFormat, PlexOutput};
pub use sonarr::{SonarrFormat, SonarrOutput};

/// Trait for formatting parsed releases into consumer-specific output.
///
/// Implementors of this trait define how to transform a
/// [`crate::model::ParsedRelease`] into a format suitable for a specific
/// media management system or platform.
pub trait OutputFormat {
    /// The output type produced by this formatter.
    type Output;

    /// Format a parsed release into the consumer-specific output.
    ///
    /// # Arguments
    ///
    /// * `release` - The parsed release to format
    ///
    /// # Returns
    ///
    /// The formatted output suitable for the target system.
    fn format(&self, release: &crate::model::ParsedRelease) -> Self::Output;
}
