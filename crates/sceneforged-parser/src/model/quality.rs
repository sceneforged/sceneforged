//! Quality-related enums for video resolution, source, and quality modifiers.

use super::ParseError;

/// Video resolution of the release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Resolution {
    /// 360p (640x360)
    _360p,
    /// 480p SD (720x480 or 854x480)
    _480p,
    /// 576p PAL SD (720x576 or 1024x576)
    _576p,
    /// 720p HD
    _720p,
    /// 1080p Full HD
    _1080p,
    /// 1440p QHD (2K)
    _1440p,
    /// 2160p Ultra HD (4K)
    _2160p,
    /// 4320p (8K)
    _4320p,
}

impl std::fmt::Display for Resolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Resolution::_360p => write!(f, "360p"),
            Resolution::_480p => write!(f, "480p"),
            Resolution::_576p => write!(f, "576p"),
            Resolution::_720p => write!(f, "720p"),
            Resolution::_1080p => write!(f, "1080p"),
            Resolution::_1440p => write!(f, "1440p"),
            Resolution::_2160p => write!(f, "2160p"),
            Resolution::_4320p => write!(f, "4320p"),
        }
    }
}

impl std::str::FromStr for Resolution {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "360p" => Ok(Resolution::_360p),
            "480p" => Ok(Resolution::_480p),
            "576p" => Ok(Resolution::_576p),
            "720p" => Ok(Resolution::_720p),
            "1080p" => Ok(Resolution::_1080p),
            "1440p" => Ok(Resolution::_1440p),
            "2160p" | "4k" => Ok(Resolution::_2160p),
            "4320p" | "8k" => Ok(Resolution::_4320p),
            _ => Err(ParseError(format!("invalid resolution: {}", s))),
        }
    }
}

/// Source/origin of the media release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Source {
    /// Blu-ray disc rip
    BluRay,
    /// Web download (lossless from streaming service)
    WebDl,
    /// Web rip (screen capture from streaming service)
    WebRip,
    /// HDTV broadcast capture
    Hdtv,
    /// DVD rip
    Dvd,
    /// DVD rip (alternative naming)
    DvdRip,
    /// Blu-ray disc rip (lower quality)
    BdRip,
    /// HD Rip (generic HD source)
    HdRip,
    /// Standard definition TV broadcast
    Sdtv,
    /// Pure digital source TV
    Pdtv,
    /// Pay-Per-View broadcast
    Ppv,
    /// Digital satellite rip
    Dsr,
    /// TV rip (generic)
    TvRip,
    /// Camera recording from theater
    Cam,
    /// HD camera recording from theater
    HdCam,
    /// Telesync (audio from external source)
    Telesync,
    /// HD Telesync
    HdTelesync,
    /// Telecine (film reel transfer)
    Telecine,
    /// Screener copy
    Screener,
    /// Regional/R5 release
    Regional,
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Source::BluRay => write!(f, "BluRay"),
            Source::WebDl => write!(f, "WEB-DL"),
            Source::WebRip => write!(f, "WEBRip"),
            Source::Hdtv => write!(f, "HDTV"),
            Source::Dvd => write!(f, "DVD"),
            Source::DvdRip => write!(f, "DVDRip"),
            Source::BdRip => write!(f, "BDRip"),
            Source::HdRip => write!(f, "HDRip"),
            Source::Sdtv => write!(f, "SDTV"),
            Source::Pdtv => write!(f, "PDTV"),
            Source::Ppv => write!(f, "PPV"),
            Source::Dsr => write!(f, "DSR"),
            Source::TvRip => write!(f, "TVRip"),
            Source::Cam => write!(f, "CAM"),
            Source::HdCam => write!(f, "HDCAM"),
            Source::Telesync => write!(f, "TS"),
            Source::HdTelesync => write!(f, "HDTS"),
            Source::Telecine => write!(f, "TC"),
            Source::Screener => write!(f, "SCR"),
            Source::Regional => write!(f, "R5"),
        }
    }
}

impl std::str::FromStr for Source {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "bluray" | "blu-ray" => Ok(Source::BluRay),
            "web-dl" | "webdl" => Ok(Source::WebDl),
            "webrip" | "web-rip" => Ok(Source::WebRip),
            "hdtv" => Ok(Source::Hdtv),
            "dvd" => Ok(Source::Dvd),
            "dvdrip" | "dvd-rip" => Ok(Source::DvdRip),
            "bdrip" | "brrip" => Ok(Source::BdRip),
            "hdrip" | "hd-rip" => Ok(Source::HdRip),
            "sdtv" => Ok(Source::Sdtv),
            "pdtv" => Ok(Source::Pdtv),
            "ppv" => Ok(Source::Ppv),
            "dsr" | "dsrip" => Ok(Source::Dsr),
            "tvrip" | "tv-rip" => Ok(Source::TvRip),
            "cam" | "camrip" => Ok(Source::Cam),
            "hdcam" => Ok(Source::HdCam),
            "ts" | "telesync" => Ok(Source::Telesync),
            "hdts" | "hdtelesync" => Ok(Source::HdTelesync),
            "tc" | "telecine" => Ok(Source::Telecine),
            "scr" | "screener" | "dvdscr" => Ok(Source::Screener),
            "r5" | "regional" => Ok(Source::Regional),
            _ => Err(ParseError(format!("invalid source: {}", s))),
        }
    }
}

/// Quality modifier indicating special release qualities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum QualityModifier {
    /// Remux (lossless copy from disc)
    Remux,
    /// Full Blu-ray disc structure
    BrDisk,
    /// Raw HD capture
    RawHd,
}

impl std::fmt::Display for QualityModifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QualityModifier::Remux => write!(f, "REMUX"),
            QualityModifier::BrDisk => write!(f, "BDMV"),
            QualityModifier::RawHd => write!(f, "RawHD"),
        }
    }
}

impl std::str::FromStr for QualityModifier {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "remux" => Ok(QualityModifier::Remux),
            "bdmv" | "brdisk" | "br-disk" => Ok(QualityModifier::BrDisk),
            "rawhd" | "raw-hd" => Ok(QualityModifier::RawHd),
            _ => Err(ParseError(format!("invalid quality modifier: {}", s))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_display_fromstr_roundtrip() {
        let variants = [
            Resolution::_360p,
            Resolution::_480p,
            Resolution::_576p,
            Resolution::_720p,
            Resolution::_1080p,
            Resolution::_1440p,
            Resolution::_2160p,
            Resolution::_4320p,
        ];
        for variant in variants {
            let s = variant.to_string();
            let parsed: Resolution = s.parse().expect("should parse");
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn source_display_fromstr_roundtrip() {
        let variants = [
            Source::BluRay,
            Source::WebDl,
            Source::WebRip,
            Source::Hdtv,
            Source::Dvd,
            Source::DvdRip,
            Source::BdRip,
            Source::HdRip,
            Source::Sdtv,
            Source::Pdtv,
            Source::Ppv,
            Source::Dsr,
            Source::TvRip,
            Source::Cam,
            Source::HdCam,
            Source::Telesync,
            Source::HdTelesync,
            Source::Telecine,
            Source::Screener,
            Source::Regional,
        ];
        for variant in variants {
            let s = variant.to_string();
            let parsed: Source = s.parse().expect("should parse");
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn quality_modifier_display_fromstr_roundtrip() {
        let variants = [
            QualityModifier::Remux,
            QualityModifier::BrDisk,
            QualityModifier::RawHd,
        ];
        for variant in variants {
            let s = variant.to_string();
            let parsed: QualityModifier = s.parse().expect("should parse");
            assert_eq!(variant, parsed);
        }
    }
}
