//! Edition flags for special releases.

/// Edition flags for special release versions.
///
/// Tracks various special edition markers commonly found in media releases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Edition {
    /// Director's Cut version
    pub directors_cut: bool,
    /// Extended/Uncut version
    pub extended: bool,
    /// Unrated version
    pub unrated: bool,
    /// IMAX version
    pub imax: bool,
    /// Theatrical release version
    pub theatrical: bool,
    /// Remastered version
    pub remastered: bool,
    /// Despecialized (fan edit restoring original)
    pub despecialized: bool,
    /// Anniversary edition
    pub anniversary: bool,
    /// Criterion Collection release
    pub criterion: bool,
    /// Collector's Edition
    pub collectors: bool,
    /// Limited Edition
    pub limited: bool,
    /// Special Edition
    pub special: bool,
}

impl Edition {
    /// Returns true if no edition flags are set.
    pub fn is_empty(&self) -> bool {
        !self.directors_cut
            && !self.extended
            && !self.unrated
            && !self.imax
            && !self.theatrical
            && !self.remastered
            && !self.despecialized
            && !self.anniversary
            && !self.criterion
            && !self.collectors
            && !self.limited
            && !self.special
    }

    /// Returns a list of edition names that are set.
    pub fn to_vec(&self) -> Vec<&'static str> {
        let mut editions = Vec::new();
        if self.directors_cut {
            editions.push("Director's Cut");
        }
        if self.extended {
            editions.push("Extended");
        }
        if self.unrated {
            editions.push("Unrated");
        }
        if self.imax {
            editions.push("IMAX");
        }
        if self.theatrical {
            editions.push("Theatrical");
        }
        if self.remastered {
            editions.push("Remastered");
        }
        if self.despecialized {
            editions.push("Despecialized");
        }
        if self.anniversary {
            editions.push("Anniversary");
        }
        if self.criterion {
            editions.push("Criterion");
        }
        if self.collectors {
            editions.push("Collector's Edition");
        }
        if self.limited {
            editions.push("Limited");
        }
        if self.special {
            editions.push("Special Edition");
        }
        editions
    }
}

impl std::fmt::Display for Edition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let editions = self.to_vec();
        if editions.is_empty() {
            write!(f, "Standard")
        } else {
            write!(f, "{}", editions.join(" "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edition_default_is_empty() {
        let edition = Edition::default();
        assert!(edition.is_empty());
        assert_eq!(edition.to_string(), "Standard");
    }

    #[test]
    fn edition_with_flags() {
        let edition = Edition {
            directors_cut: true,
            extended: true,
            ..Default::default()
        };
        assert!(!edition.is_empty());
        assert_eq!(edition.to_vec(), vec!["Director's Cut", "Extended"]);
    }

    #[test]
    fn edition_display() {
        let edition = Edition {
            imax: true,
            remastered: true,
            ..Default::default()
        };
        assert_eq!(edition.to_string(), "IMAX Remastered");
    }
}
