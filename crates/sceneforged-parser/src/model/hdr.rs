//! HDR format enum.

use super::ParseError;

/// High Dynamic Range format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HdrFormat {
    /// HDR10 (static metadata)
    Hdr10,
    /// HDR10+ (dynamic metadata)
    Hdr10Plus,
    /// Dolby Vision
    DolbyVision,
    /// Dolby Vision with HDR10 fallback
    DolbyVisionHdr10,
    /// Hybrid Log-Gamma
    Hlg,
}

impl std::fmt::Display for HdrFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HdrFormat::Hdr10 => write!(f, "HDR10"),
            HdrFormat::Hdr10Plus => write!(f, "HDR10+"),
            HdrFormat::DolbyVision => write!(f, "DV"),
            HdrFormat::DolbyVisionHdr10 => write!(f, "DV HDR10"),
            HdrFormat::Hlg => write!(f, "HLG"),
        }
    }
}

impl std::str::FromStr for HdrFormat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "hdr10" | "hdr" => Ok(HdrFormat::Hdr10),
            "hdr10+" | "hdr10plus" => Ok(HdrFormat::Hdr10Plus),
            "dv" | "dolbyvision" | "dolby vision" => Ok(HdrFormat::DolbyVision),
            "dv hdr10" | "dv.hdr10" | "dolbyvision hdr10" => Ok(HdrFormat::DolbyVisionHdr10),
            "hlg" => Ok(HdrFormat::Hlg),
            _ => Err(ParseError(format!("invalid HDR format: {}", s))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hdr_format_display_fromstr_roundtrip() {
        let variants = [
            HdrFormat::Hdr10,
            HdrFormat::Hdr10Plus,
            HdrFormat::DolbyVision,
            HdrFormat::DolbyVisionHdr10,
            HdrFormat::Hlg,
        ];
        for variant in variants {
            let s = variant.to_string();
            let parsed: HdrFormat = s.parse().expect("should parse");
            assert_eq!(variant, parsed);
        }
    }
}
