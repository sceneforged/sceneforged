//! Media file probing for the scanner.
//!
//! This module wraps the sceneforged-probe/sceneforged-av probe functionality
//! for use in the scanner module.

use crate::probe::MediaInfo;
use anyhow::Result;
use std::path::Path;

/// File prober for extracting media information.
pub struct FileProber {
    // Future: could add caching, backend preferences, etc.
}

impl FileProber {
    /// Create a new file prober.
    pub fn new() -> Self {
        Self {}
    }

    /// Probe a media file and return its information.
    pub fn probe(&self, path: &Path) -> Result<MediaInfo> {
        crate::probe::probe_file(path)
    }
}

impl Default for FileProber {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prober_creation() {
        // Verify the prober can be created and used
        let prober = FileProber::new();
        let default = FileProber::default();
        // Both creation methods should work
        let _ = (prober, default);
    }
}
