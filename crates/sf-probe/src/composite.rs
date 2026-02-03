//! A composite prober that delegates to multiple [`Prober`] implementations.

use std::path::Path;

use crate::prober::Prober;
use crate::types::MediaInfo;

/// Tries each registered [`Prober`] in order and returns the first successful result.
///
/// This allows layering multiple prober strategies (e.g., a fast Rust prober
/// with a fallback to an ffprobe-based prober).
pub struct CompositeProber {
    probers: Vec<Box<dyn Prober>>,
}

impl CompositeProber {
    /// Create a new `CompositeProber` from an ordered list of probers.
    ///
    /// Probers are tried in the order provided. The first prober whose
    /// [`Prober::supports`] returns `true` and whose [`Prober::probe`] succeeds
    /// will have its result returned.
    pub fn new(probers: Vec<Box<dyn Prober>>) -> Self {
        Self { probers }
    }
}

impl Prober for CompositeProber {
    fn name(&self) -> &'static str {
        "composite"
    }

    fn supports(&self, path: &Path) -> bool {
        self.probers.iter().any(|p| p.supports(path))
    }

    fn probe(&self, path: &Path) -> sf_core::Result<MediaInfo> {
        let mut last_err = None;

        for prober in &self.probers {
            if !prober.supports(path) {
                continue;
            }

            match prober.probe(path) {
                Ok(info) => return Ok(info),
                Err(e) => {
                    tracing::debug!(
                        prober = prober.name(),
                        error = %e,
                        "prober failed, trying next"
                    );
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            sf_core::Error::Probe(format!(
                "no prober supports file: {}",
                path.display()
            ))
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rust_prober::RustProber;

    #[test]
    fn composite_supports_delegates() {
        let composite = CompositeProber::new(vec![Box::new(RustProber::new())]);
        assert!(composite.supports(Path::new("movie.mkv")));
        assert!(composite.supports(Path::new("movie.mp4")));
        assert!(!composite.supports(Path::new("movie.avi")));
    }

    #[test]
    fn composite_no_probers_returns_error() {
        let composite = CompositeProber::new(vec![]);
        let result = composite.probe(Path::new("movie.mkv"));
        assert!(result.is_err());
    }
}
