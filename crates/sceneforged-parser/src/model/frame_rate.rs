//! Frame rate enum.

use super::ParseError;

/// Video frame rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FrameRate {
    /// 23.976 fps (NTSC film)
    _23_976,
    /// 24 fps (film)
    _24,
    /// 25 fps (PAL)
    _25,
    /// 29.97 fps (NTSC video)
    _29_97,
    /// 30 fps
    _30,
    /// 50 fps (PAL high frame rate)
    _50,
    /// 59.94 fps (NTSC high frame rate)
    _59_94,
    /// 60 fps
    _60,
}

impl std::fmt::Display for FrameRate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameRate::_23_976 => write!(f, "23.976fps"),
            FrameRate::_24 => write!(f, "24fps"),
            FrameRate::_25 => write!(f, "25fps"),
            FrameRate::_29_97 => write!(f, "29.97fps"),
            FrameRate::_30 => write!(f, "30fps"),
            FrameRate::_50 => write!(f, "50fps"),
            FrameRate::_59_94 => write!(f, "59.94fps"),
            FrameRate::_60 => write!(f, "60fps"),
        }
    }
}

impl std::str::FromStr for FrameRate {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_lowercase();
        let s = s.trim_end_matches("fps");
        match s {
            "23.976" | "23.98" => Ok(FrameRate::_23_976),
            "24" => Ok(FrameRate::_24),
            "25" => Ok(FrameRate::_25),
            "29.97" => Ok(FrameRate::_29_97),
            "30" => Ok(FrameRate::_30),
            "50" => Ok(FrameRate::_50),
            "59.94" => Ok(FrameRate::_59_94),
            "60" => Ok(FrameRate::_60),
            _ => Err(ParseError(format!("invalid frame rate: {}", s))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_rate_display_fromstr_roundtrip() {
        let variants = [
            FrameRate::_23_976,
            FrameRate::_24,
            FrameRate::_25,
            FrameRate::_29_97,
            FrameRate::_30,
            FrameRate::_50,
            FrameRate::_59_94,
            FrameRate::_60,
        ];
        for variant in variants {
            let s = variant.to_string();
            let parsed: FrameRate = s.parse().expect("should parse");
            assert_eq!(variant, parsed);
        }
    }
}
