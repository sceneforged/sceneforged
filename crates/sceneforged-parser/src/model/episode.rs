//! Episode-related types including revision information.

/// Revision information for release versions.
///
/// Tracks the version number (v2, v3, etc.) and REAL/PROPER count
/// for re-releases that fix issues in previous versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Revision {
    /// Version number (default 1, e.g., v2 = 2)
    pub version: u8,
    /// Number of REAL/PROPER tags (default 0)
    pub real: u8,
}

impl Default for Revision {
    fn default() -> Self {
        Self {
            version: 1,
            real: 0,
        }
    }
}

impl std::fmt::Display for Revision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.version > 1 {
            write!(f, "v{}", self.version)?;
        }
        if self.real > 0 {
            if self.version > 1 {
                write!(f, " ")?;
            }
            for _ in 0..self.real {
                write!(f, "PROPER")?;
            }
        }
        if self.version == 1 && self.real == 0 {
            write!(f, "v1")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revision_default() {
        let rev = Revision::default();
        assert_eq!(rev.version, 1);
        assert_eq!(rev.real, 0);
    }

    #[test]
    fn revision_display() {
        assert_eq!(
            Revision {
                version: 1,
                real: 0
            }
            .to_string(),
            "v1"
        );
        assert_eq!(
            Revision {
                version: 2,
                real: 0
            }
            .to_string(),
            "v2"
        );
        assert_eq!(
            Revision {
                version: 1,
                real: 1
            }
            .to_string(),
            "PROPER"
        );
        assert_eq!(
            Revision {
                version: 2,
                real: 1
            }
            .to_string(),
            "v2 PROPER"
        );
    }
}
