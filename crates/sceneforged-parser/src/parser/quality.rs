//! Quality metadata parser.
//!
//! Extracts resolution, source, and bit depth from the token stream.

use crate::lexer::{Lexer, Token};
use crate::model::{Confidence, ParsedField, ParsedRelease, Resolution, Source};

/// Extract quality information from the token stream.
///
/// This parser looks for resolution, source, and bit depth tokens and populates
/// the corresponding fields in the ParsedRelease.
pub fn extract(lexer: &Lexer, release: &mut ParsedRelease) {
    let tokens = lexer.tokens();

    for (token, span) in tokens {
        match token {
            Token::Resolution(text) => {
                if release.resolution.is_none() {
                    if let Some(res) = parse_resolution(text) {
                        release.resolution = Some(ParsedField::new(
                            res,
                            Confidence::CERTAIN,
                            (span.start, span.end),
                            *text,
                        ));
                    }
                }
            }
            Token::SourceBdRip(text) => {
                // BDRip/BRRip are distinct from BluRay
                if release.source.is_none()
                    || matches!(release.source.as_ref().map(|s| s.value), Some(Source::Ppv))
                {
                    release.source = Some(ParsedField::new(
                        Source::BdRip,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::SourceBluray(text) => {
                // Higher priority sources override PPV
                if release.source.is_none()
                    || matches!(release.source.as_ref().map(|s| s.value), Some(Source::Ppv))
                {
                    release.source = Some(ParsedField::new(
                        Source::BluRay,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::SourceWebDL(text) => {
                // Higher priority sources override PPV
                if release.source.is_none()
                    || matches!(release.source.as_ref().map(|s| s.value), Some(Source::Ppv))
                {
                    release.source = Some(ParsedField::new(
                        Source::WebDl,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::SourceWebRip(text) => {
                // Higher priority sources override PPV
                if release.source.is_none()
                    || matches!(release.source.as_ref().map(|s| s.value), Some(Source::Ppv))
                {
                    release.source = Some(ParsedField::new(
                        Source::WebRip,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::SourceHD(text) => {
                // Higher priority sources override PPV
                if release.source.is_none()
                    || matches!(release.source.as_ref().map(|s| s.value), Some(Source::Ppv))
                {
                    let source = if text.to_uppercase().contains("HDTV") {
                        Source::Hdtv
                    } else {
                        Source::HdRip
                    };
                    release.source = Some(ParsedField::new(
                        source,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::SourceDVD(text) => {
                if release.source.is_none() {
                    release.source = Some(ParsedField::new(
                        Source::Dvd,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::SourceCam(text) => {
                if release.source.is_none() {
                    let source = if text.to_uppercase().contains("HD") {
                        Source::HdCam
                    } else {
                        Source::Cam
                    };
                    release.source = Some(ParsedField::new(
                        source,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::SourceTelesync(text) => {
                if release.source.is_none() {
                    let source = if text.to_uppercase().contains("HD") {
                        Source::HdTelesync
                    } else {
                        Source::Telesync
                    };
                    release.source = Some(ParsedField::new(
                        source,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::SourceTelecine(text) => {
                if release.source.is_none() {
                    release.source = Some(ParsedField::new(
                        Source::Telecine,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::SourceScreener(text) => {
                if release.source.is_none() {
                    let upper = text.to_uppercase();
                    let source = if upper.contains("R5") {
                        Source::Regional
                    } else {
                        Source::Screener
                    };
                    release.source = Some(ParsedField::new(
                        source,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::SourcePPV(text) => {
                // PPV is a lower priority source - only use if no other source is set
                // or if it appears after other sources (prefer WEB-DL, HDTV, etc.)
                if release.source.is_none() {
                    release.source = Some(ParsedField::new(
                        Source::Ppv,
                        Confidence::MEDIUM, // Lower confidence to allow override
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::SourceTV(text) => {
                if release.source.is_none() {
                    let upper = text.to_uppercase();
                    let source = if upper.contains("PDTV") {
                        Source::Pdtv
                    } else if upper.contains("SDTV") {
                        Source::Sdtv
                    } else {
                        Source::Dsr
                    };
                    release.source = Some(ParsedField::new(
                        source,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::SourceWeb(text) => {
                if release.source.is_none() {
                    release.source = Some(ParsedField::new(
                        Source::WebDl, // Default bare WEB to WEB-DL
                        Confidence::HIGH,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::SourceBD(text) => {
                if release.source.is_none() {
                    release.source = Some(ParsedField::new(
                        Source::BluRay,
                        Confidence::HIGH,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::QualityRemux(text) => {
                if release.quality_modifier.is_none() {
                    release.quality_modifier = Some(ParsedField::new(
                        crate::model::QualityModifier::Remux,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::BitDepth10(text) => {
                if release.bit_depth.is_none() {
                    release.bit_depth = Some(ParsedField::new(
                        10,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::BitDepth8(text) => {
                if release.bit_depth.is_none() {
                    release.bit_depth = Some(ParsedField::new(
                        8,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::Word(text) => {
                // Check for additional resolution keywords
                let upper = text.to_uppercase();
                if upper == "UHD" || upper == "4K" {
                    if release.resolution.is_none() {
                        release.resolution = Some(ParsedField::new(
                            Resolution::_2160p,
                            Confidence::HIGH,
                            (span.start, span.end),
                            *text,
                        ));
                    }
                } else if upper == "8K" {
                    if release.resolution.is_none() {
                        release.resolution = Some(ParsedField::new(
                            Resolution::_4320p,
                            Confidence::HIGH,
                            (span.start, span.end),
                            *text,
                        ));
                    }
                }
            }
            _ => {}
        }
    }
}

/// Parse a resolution string into a Resolution enum.
///
/// Supports formats like "1080p", "720p", "2160p", "1920x1080", etc.
fn parse_resolution(text: &str) -> Option<Resolution> {
    let upper = text.to_uppercase();

    // Handle pixel dimensions (e.g., 1920x1080)
    if upper.contains('X') {
        if upper.contains("3840") || upper.contains("2160") {
            return Some(Resolution::_2160p);
        } else if upper.contains("1920") || upper.contains("1080") {
            return Some(Resolution::_1080p);
        } else if upper.contains("1280") || upper.contains("720") {
            return Some(Resolution::_720p);
        }
    }

    // Handle standard format (e.g., 1080p, 720p)
    let digits: String = upper.chars().filter(|c| c.is_ascii_digit()).collect();

    match digits.as_str() {
        "360" => Some(Resolution::_360p),
        "480" => Some(Resolution::_480p),
        "576" => Some(Resolution::_576p),
        "720" => Some(Resolution::_720p),
        "1080" => Some(Resolution::_1080p),
        "2160" => Some(Resolution::_2160p),
        "4320" => Some(Resolution::_4320p),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_resolution() {
        assert_eq!(parse_resolution("1080p"), Some(Resolution::_1080p));
        assert_eq!(parse_resolution("720p"), Some(Resolution::_720p));
        assert_eq!(parse_resolution("2160p"), Some(Resolution::_2160p));
        assert_eq!(parse_resolution("1080"), Some(Resolution::_1080p));
    }

    #[test]
    fn test_extract_resolution() {
        let input = "Movie.2020.1080p.BluRay.x264-GROUP";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        extract(&lexer, &mut release);

        assert!(release.resolution.is_some());
        assert_eq!(**release.resolution.as_ref().unwrap(), Resolution::_1080p);
    }

    #[test]
    fn test_extract_source() {
        let input = "Movie.2020.1080p.BluRay.x264-GROUP";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        extract(&lexer, &mut release);

        assert!(release.source.is_some());
        assert_eq!(**release.source.as_ref().unwrap(), Source::BluRay);
    }

    #[test]
    fn test_extract_webdl() {
        let input = "Movie.2020.1080p.WEB-DL.x264-GROUP";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        extract(&lexer, &mut release);

        assert!(release.source.is_some());
        assert_eq!(**release.source.as_ref().unwrap(), Source::WebDl);
    }

    #[test]
    fn test_extract_bit_depth() {
        let input = "Movie.2020.2160p.BluRay.10bit.x265-GROUP";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        extract(&lexer, &mut release);

        assert!(release.bit_depth.is_some());
        assert_eq!(**release.bit_depth.as_ref().unwrap(), 10);
    }
}
