//! Media type enum.

use super::ParseError;

/// Type of media content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MediaType {
    /// Movie/Film
    Movie,
    /// TV Series
    Tv,
    /// Anime (Japanese animation)
    Anime,
    /// Unknown or undetermined type
    #[default]
    Unknown,
}

impl std::fmt::Display for MediaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MediaType::Movie => write!(f, "Movie"),
            MediaType::Tv => write!(f, "TV"),
            MediaType::Anime => write!(f, "Anime"),
            MediaType::Unknown => write!(f, "Unknown"),
        }
    }
}

impl std::str::FromStr for MediaType {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "movie" | "film" => Ok(MediaType::Movie),
            "tv" | "series" | "show" => Ok(MediaType::Tv),
            "anime" => Ok(MediaType::Anime),
            "unknown" => Ok(MediaType::Unknown),
            _ => Err(ParseError(format!("invalid media type: {}", s))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_type_display_fromstr_roundtrip() {
        let variants = [
            MediaType::Movie,
            MediaType::Tv,
            MediaType::Anime,
            MediaType::Unknown,
        ];
        for variant in variants {
            let s = variant.to_string();
            let parsed: MediaType = s.parse().expect("should parse");
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn media_type_default() {
        assert_eq!(MediaType::default(), MediaType::Unknown);
    }
}
