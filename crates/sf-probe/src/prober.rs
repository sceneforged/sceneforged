//! The [`Prober`] trait defining the interface for media file probing.

use std::path::Path;

use crate::types::MediaInfo;

/// A media file prober capable of extracting metadata from video files.
///
/// Implementations must be safe to share across threads (`Send + Sync`).
pub trait Prober: Send + Sync {
    /// Human-readable name identifying this prober implementation.
    fn name(&self) -> &'static str;

    /// Probe a media file at the given path and extract metadata.
    ///
    /// Returns a [`MediaInfo`] on success, or an error if the file cannot
    /// be read or parsed.
    fn probe(&self, path: &Path) -> sf_core::Result<MediaInfo>;

    /// Check whether this prober supports the given file path.
    ///
    /// Typically checks the file extension or magic bytes. A return value
    /// of `true` does not guarantee that [`Prober::probe`] will succeed.
    fn supports(&self, path: &Path) -> bool;
}
