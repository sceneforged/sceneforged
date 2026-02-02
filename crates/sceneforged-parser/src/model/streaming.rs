//! Streaming service enum.

use super::ParseError;

/// Streaming service origin of the release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StreamingService {
    /// Netflix
    Netflix,
    /// Amazon Prime Video
    Amazon,
    /// Apple TV+
    AppleTv,
    /// Disney+
    DisneyPlus,
    /// Hulu
    Hulu,
    /// HBO Max
    HboMax,
    /// Peacock
    Peacock,
    /// Paramount+
    Paramount,
    /// Crunchyroll
    Crunchyroll,
    /// HIDIVE
    Hidive,
    /// iTunes
    ITunes,
    /// YouTube Premium/Red
    YouTubeRed,
    /// Stan (Australian)
    Stan,
    /// Channel 4 (UK)
    All4,
    /// NOW (UK/Ireland)
    Now,
}

impl std::fmt::Display for StreamingService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamingService::Netflix => write!(f, "Netflix"),
            StreamingService::Amazon => write!(f, "Amazon"),
            StreamingService::AppleTv => write!(f, "AppleTv"),
            StreamingService::DisneyPlus => write!(f, "DisneyPlus"),
            StreamingService::Hulu => write!(f, "Hulu"),
            StreamingService::HboMax => write!(f, "HboMax"),
            StreamingService::Peacock => write!(f, "Peacock"),
            StreamingService::Paramount => write!(f, "Paramount"),
            StreamingService::Crunchyroll => write!(f, "Crunchyroll"),
            StreamingService::Hidive => write!(f, "Hidive"),
            StreamingService::ITunes => write!(f, "iTunes"),
            StreamingService::YouTubeRed => write!(f, "YouTubeRed"),
            StreamingService::Stan => write!(f, "Stan"),
            StreamingService::All4 => write!(f, "All4"),
            StreamingService::Now => write!(f, "Now"),
        }
    }
}

impl std::str::FromStr for StreamingService {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "nf" | "netflix" => Ok(StreamingService::Netflix),
            "amzn" | "amazon" | "prime" => Ok(StreamingService::Amazon),
            "atvp" | "atv+" | "appletv" | "apple tv+" => Ok(StreamingService::AppleTv),
            "dsnp" | "disney+" | "disneyplus" => Ok(StreamingService::DisneyPlus),
            "hulu" => Ok(StreamingService::Hulu),
            "hmax" | "hbo max" | "hbomax" => Ok(StreamingService::HboMax),
            "pcok" | "peacock" => Ok(StreamingService::Peacock),
            "pmtp" | "paramount+" | "paramountplus" | "paramount" => {
                Ok(StreamingService::Paramount)
            }
            "cr" | "crunchyroll" => Ok(StreamingService::Crunchyroll),
            "hdiv" | "hidive" => Ok(StreamingService::Hidive),
            "it" | "itunes" => Ok(StreamingService::ITunes),
            "red" | "youtube red" | "youtube premium" | "youtubered" => {
                Ok(StreamingService::YouTubeRed)
            }
            "stan" => Ok(StreamingService::Stan),
            "all4" | "4od" => Ok(StreamingService::All4),
            "now" | "nowtv" => Ok(StreamingService::Now),
            _ => Err(ParseError(format!("invalid streaming service: {}", s))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_service_display_fromstr_roundtrip() {
        let variants = [
            StreamingService::Netflix,
            StreamingService::Amazon,
            StreamingService::AppleTv,
            StreamingService::DisneyPlus,
            StreamingService::Hulu,
            StreamingService::HboMax,
            StreamingService::Peacock,
            StreamingService::Paramount,
            StreamingService::Crunchyroll,
            StreamingService::Hidive,
            StreamingService::ITunes,
            StreamingService::YouTubeRed,
            StreamingService::Stan,
            StreamingService::All4,
            StreamingService::Now,
        ];
        for variant in variants {
            let s = variant.to_string();
            let parsed: StreamingService = s.parse().expect("should parse");
            assert_eq!(variant, parsed);
        }
    }
}
