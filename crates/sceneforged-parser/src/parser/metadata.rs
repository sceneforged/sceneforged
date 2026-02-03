//! Metadata parser.
//!
//! Extracts year, release group, streaming service, edition flags,
//! and container format from the token stream.

use crate::config::{ParserConfig, YearInTitleMode};
use crate::lexer::{Lexer, Token};
use crate::model::{Confidence, ParsedField, ParsedRelease, StreamingService};

/// Extract metadata from the token stream using default config.
#[allow(dead_code)]
pub fn extract(lexer: &Lexer, release: &mut ParsedRelease) {
    extract_with_config(lexer, release, &ParserConfig::default())
}

/// Extract metadata from the token stream with custom configuration.
///
/// This parser looks for year, release group, streaming service, and other
/// metadata tokens and populates the corresponding fields in the ParsedRelease.
pub fn extract_with_config(lexer: &Lexer, release: &mut ParsedRelease, config: &ParserConfig) {
    let tokens = lexer.tokens();
    let input = lexer.input();

    // Collect all year candidates with their positions and confidence
    let mut year_candidates: Vec<(u16, usize, Confidence, std::ops::Range<usize>, &str)> =
        Vec::new();

    for (i, (token, span)) in tokens.iter().enumerate() {
        match token {
            Token::Year(text) => {
                if let Ok(year) = text.parse::<u16>() {
                    // Check if this year appears after quality/resolution indicators
                    // Years after these markers are more likely to be the actual release year
                    let is_after_quality = tokens[..i].iter().any(|(t, _)| {
                        matches!(
                            t,
                            Token::Resolution(_)
                                | Token::SourceBluray(_)
                                | Token::SourceWebDL(_)
                                | Token::SourceWebRip(_)
                                | Token::SourceHD(_)
                                | Token::SourceDVD(_)
                        )
                    });

                    // Check if IMMEDIATELY followed by season/episode marker
                    // Look past delimiters to find the next significant token
                    let mut next_significant_idx = i + 1;
                    while next_significant_idx < tokens.len() {
                        if !matches!(
                            tokens[next_significant_idx].0,
                            Token::Dot | Token::Hyphen | Token::Underscore
                        ) {
                            break;
                        }
                        next_significant_idx += 1;
                    }
                    let is_immediately_before_episodes = next_significant_idx < tokens.len()
                        && matches!(
                            tokens[next_significant_idx].0,
                            Token::SeasonEpisode(_)
                                | Token::SeasonEpisodeX(_)
                                | Token::SeasonOnly(_)
                        );

                    // Also check if immediately followed by a compressed episode number (3-4 digits)
                    let is_immediately_before_compressed = if next_significant_idx < tokens.len() {
                        if let Token::Number(num_text) = &tokens[next_significant_idx].0 {
                            // Check if it's a valid compressed episode (3-4 digits)
                            let len = num_text.len();
                            (len == 3 || len == 4) && num_text.chars().all(|c| c.is_ascii_digit())
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    // Based on config, decide whether to skip years immediately before episodes
                    // IncludeInTitle: skip year (it becomes part of title)
                    // TreatAsMetadata: extract year as metadata
                    if config.year_in_title == YearInTitleMode::IncludeInTitle
                        && (is_immediately_before_episodes || is_immediately_before_compressed)
                    {
                        continue;
                    }

                    let is_before_episodes = tokens
                        .iter()
                        .skip(i)
                        .any(|(t, _)| matches!(t, Token::SeasonEpisode(_) | Token::SeasonOnly(_)));

                    let confidence = if is_after_quality {
                        Confidence::CERTAIN // Year after quality markers is most reliable
                    } else if is_before_episodes {
                        Confidence::HIGH // Year before episodes is likely metadata
                    } else {
                        Confidence::MEDIUM // Could be part of title
                    };

                    year_candidates.push((year, i, confidence, span.clone(), *text));
                }
            }

            Token::StreamingService(text) => {
                if release.streaming_service.is_none() {
                    if let Some(service) = parse_streaming_service(text) {
                        release.streaming_service = Some(ParsedField::new(
                            service,
                            Confidence::CERTAIN,
                            (span.start, span.end),
                            *text,
                        ));
                    }
                }
            }

            Token::Edition(text) => {
                // Add to edition flags
                let upper = text.to_uppercase();
                if upper.contains("EXTENDED") {
                    release.edition.extended = true;
                } else if upper.contains("UNCUT") || upper.contains("UNRATED") {
                    release.edition.unrated = true;
                } else if upper.contains("DC") || upper.contains("DIRECTOR") {
                    release.edition.directors_cut = true;
                } else if upper.contains("THEATRICAL") {
                    release.edition.theatrical = true;
                }
            }

            Token::ReleaseModifier(text) => {
                // Update revision based on modifier
                let upper = text.to_uppercase();
                if upper == "PROPER" || upper == "REAL" {
                    release.revision.real += 1;
                } else if upper == "REPACK" || upper == "RERIP" {
                    release.revision.version += 1;
                }
            }

            Token::Word(text) => {
                // Check for container extension
                let lower = text.to_lowercase();
                if matches!(
                    lower.as_str(),
                    "mkv" | "mp4" | "avi" | "m4v" | "ts" | "m2ts"
                ) && release.container.is_none()
                {
                    release.container = Some(ParsedField::new(
                        lower.clone(),
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }

                // Check for REMUX quality modifier
                if text.to_uppercase() == "REMUX" {
                    use crate::model::QualityModifier;
                    if release.quality_modifier.is_none() {
                        release.quality_modifier = Some(ParsedField::new(
                            QualityModifier::Remux,
                            Confidence::CERTAIN,
                            (span.start, span.end),
                            *text,
                        ));
                    }
                }
            }

            // Look for release group after hyphen at end
            Token::Hyphen => {
                // Check if this is followed by a word token near the end
                if i + 1 < tokens.len() && release.release_group.is_none() {
                    if let (Token::Word(group_name), group_span) = &tokens[i + 1] {
                        // Check if this is likely the last significant token
                        let remaining_significant: Vec<_> = tokens
                            .iter()
                            .skip(i + 2)
                            .filter(|(t, _)| {
                                !matches!(
                                    t,
                                    Token::Dot
                                        | Token::Hyphen
                                        | Token::Underscore
                                        | Token::BracketClose
                                        | Token::ParenClose
                                )
                            })
                            .collect();

                        // If only container extension follows, this is likely the release group
                        let is_release_group = remaining_significant.is_empty()
                            || (remaining_significant.len() == 1
                                && matches!(remaining_significant[0].0, Token::Word(w) if
                                    matches!(w.to_lowercase().as_str(), "mkv" | "mp4" | "avi" | "m4v" | "ts" | "m2ts")));

                        if is_release_group {
                            release.release_group = Some(ParsedField::new(
                                group_name.to_string(),
                                Confidence::HIGH,
                                (group_span.start, group_span.end),
                                *group_name,
                            ));
                        }
                    }
                }
            }

            // Look for release group in brackets (anime style)
            Token::BracketOpen => {
                // Check if this is at/near the beginning
                if i < 3 && i + 1 < tokens.len() && release.release_group.is_none() {
                    // Collect all words/hyphens until the closing bracket
                    let mut group_parts = Vec::new();
                    let mut j = i + 1;
                    let start_pos = tokens[i + 1].1.start;
                    let mut end_pos = tokens[i + 1].1.end;

                    while j < tokens.len() {
                        match &tokens[j].0 {
                            Token::Word(w) => {
                                group_parts.push(*w);
                                end_pos = tokens[j].1.end;
                                j += 1;
                            }
                            Token::Hyphen => {
                                // Include hyphen in multi-word groups like "Erai-raws"
                                if !group_parts.is_empty() && j + 1 < tokens.len() {
                                    if let Token::Word(_) = tokens[j + 1].0 {
                                        group_parts.push("-");
                                        j += 1;
                                    } else {
                                        break;
                                    }
                                } else {
                                    break;
                                }
                            }
                            Token::BracketClose => {
                                // Found the closing bracket
                                if !group_parts.is_empty() {
                                    let group_name = group_parts.join("");
                                    release.release_group = Some(ParsedField::new(
                                        group_name,
                                        Confidence::HIGH,
                                        (start_pos, end_pos),
                                        &input[start_pos..end_pos],
                                    ));
                                }
                                break;
                            }
                            _ => break,
                        }
                    }
                }

                // Check for CRC32 checksums in brackets (8 hex chars)
                if i + 1 < tokens.len() {
                    if let (Token::Word(checksum), checksum_span) = &tokens[i + 1] {
                        if checksum.len() == 8
                            && checksum.chars().all(|c| c.is_ascii_hexdigit())
                            && release.file_checksum.is_none()
                            && i + 2 < tokens.len()
                            && matches!(tokens[i + 2].0, Token::BracketClose)
                        {
                            release.file_checksum = Some(ParsedField::new(
                                checksum.to_uppercase(),
                                Confidence::CERTAIN,
                                (checksum_span.start, checksum_span.end),
                                *checksum,
                            ));

                            // CRC32 is a strong indicator of anime
                            if *release.media_type == crate::model::MediaType::Unknown {
                                release.media_type = ParsedField::new(
                                    crate::model::MediaType::Anime,
                                    Confidence::HIGH,
                                    (checksum_span.start, checksum_span.end),
                                    *checksum,
                                );
                            }
                        }
                    }
                }
            }

            _ => {}
        }
    }

    // Look for release group at the end in brackets (e.g., "...x264-GROUP[ettv]")
    // This is an alternative release group pattern where the group after a hyphen
    // is followed by a bracketed tracker/uploader name
    if release.release_group.is_none() {
        // Scan backwards from the end looking for [Word] pattern
        let mut i = tokens.len();
        while i > 0 {
            i -= 1;
            if let Token::BracketClose = &tokens[i].0 {
                // Found closing bracket, look for opening bracket and word before it
                if i >= 2 {
                    // Check if we have [Word] pattern
                    if let Token::Word(_bracket_word) = &tokens[i - 1].0 {
                        if let Token::BracketOpen = &tokens[i - 2].0 {
                            // Found [word] at the end, now look for Hyphen-Word before it
                            if i >= 4 {
                                if let Token::Word(group_name) = &tokens[i - 3].0 {
                                    if let Token::Hyphen = &tokens[i - 4].0 {
                                        // Found -GROUP[word] pattern
                                        let group_span = &tokens[i - 3].1;
                                        release.release_group = Some(ParsedField::new(
                                            group_name.to_string(),
                                            Confidence::HIGH,
                                            (group_span.start, group_span.end),
                                            *group_name,
                                        ));
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Select the best year candidate
    // Prefer years with higher confidence (after quality markers > before episodes > standalone)
    if release.year.is_none() && !year_candidates.is_empty() {
        // Sort by confidence (highest first), then by position (latest first)
        year_candidates.sort_by(|a, b| {
            b.2.partial_cmp(&a.2)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.1.cmp(&a.1))
        });
        let (year, _, confidence, span, text) = &year_candidates[0];
        release.year = Some(ParsedField::new(
            *year,
            *confidence,
            (span.start, span.end),
            *text,
        ));
    }

    // Extract container from filename if present
    if release.container.is_none() {
        if let Some(dot_pos) = input.rfind('.') {
            let ext = &input[dot_pos + 1..];
            if matches!(
                ext.to_lowercase().as_str(),
                "mkv" | "mp4" | "avi" | "m4v" | "ts" | "m2ts"
            ) {
                let ext_lower = ext.to_lowercase();
                release.container = Some(ParsedField::new(
                    ext_lower,
                    Confidence::CERTAIN,
                    (dot_pos + 1, input.len()),
                    ext,
                ));
            }
        }
    }
}

/// Parse a streaming service abbreviation into a StreamingService enum.
fn parse_streaming_service(text: &str) -> Option<StreamingService> {
    let upper = text.to_uppercase();
    match upper.as_str() {
        "AMZN" => Some(StreamingService::Amazon),
        "NF" => Some(StreamingService::Netflix),
        "DSNP" => Some(StreamingService::DisneyPlus),
        "HMAX" => Some(StreamingService::HboMax),
        "ATVP" => Some(StreamingService::AppleTv),
        "PMTP" => Some(StreamingService::Paramount),
        "STAN" => Some(StreamingService::Stan),
        "PCOK" => Some(StreamingService::Peacock),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_year() {
        let input = "Movie.Title.2024.1080p.BluRay.x264-GROUP";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        extract(&lexer, &mut release);

        assert!(release.year.is_some());
        assert_eq!(**release.year.as_ref().unwrap(), 2024);
    }

    #[test]
    fn test_extract_release_group() {
        let input = "Movie.Title.2024.1080p.BluRay.x264-GROUP";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        extract(&lexer, &mut release);

        assert!(release.release_group.is_some());
        assert_eq!(**release.release_group.as_ref().unwrap(), "GROUP");
    }

    #[test]
    fn test_extract_anime_release_group() {
        let input = "[SubGroup] Anime Title - 01 [1080p].mkv";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        extract(&lexer, &mut release);

        assert!(release.release_group.is_some());
        assert_eq!(**release.release_group.as_ref().unwrap(), "SubGroup");
    }

    #[test]
    fn test_extract_container() {
        let input = "Movie.Title.2024.1080p.BluRay.x264-GROUP.mkv";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        extract(&lexer, &mut release);

        assert!(release.container.is_some());
        assert_eq!(**release.container.as_ref().unwrap(), "mkv");
    }

    #[test]
    fn test_extract_crc32() {
        let input = "[SubGroup] Anime Title - 01 [ABCD1234].mkv";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        extract(&lexer, &mut release);

        assert!(release.file_checksum.is_some());
        assert_eq!(**release.file_checksum.as_ref().unwrap(), "ABCD1234");
    }
}
