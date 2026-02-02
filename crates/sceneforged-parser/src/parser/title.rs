//! Title extraction parser.
//!
//! Extracts the title from remaining tokens that haven't been consumed
//! by other parsers.

use super::episode::extract_embedded_episode;
use crate::config::{ParserConfig, YearInTitleMode};
use crate::lexer::{Lexer, Token};
use crate::model::{Confidence, ParsedRelease};

/// Extract the title from the token stream using default config.
#[allow(dead_code)]
pub fn extract(lexer: &Lexer, release: &mut ParsedRelease) {
    extract_with_config(lexer, release, &ParserConfig::default())
}

/// Extract the title from the token stream with custom configuration.
///
/// This parser runs last and extracts the title from tokens that appear
/// before metadata markers (year, resolution, source, etc.).
///
/// The title is typically everything before the first "stop token"
/// (year, season/episode, resolution, etc.).
pub fn extract_with_config(lexer: &Lexer, release: &mut ParsedRelease, config: &ParserConfig) {
    let tokens = lexer.tokens();
    let input = lexer.input();

    // Phase 1: Check if input starts with an episode marker - if so, title is empty
    // Skip brackets, parens, and delimiters to find first "content" token
    let first_content_token = tokens.iter().find(|(t, _)| {
        !matches!(
            t,
            Token::BracketOpen
                | Token::BracketClose
                | Token::ParenOpen
                | Token::ParenClose
                | Token::Dot
                | Token::Hyphen
                | Token::Underscore
        )
    });

    if let Some((token, span)) = first_content_token {
        if matches!(
            token,
            Token::SeasonEpisode(_)
                | Token::SeasonEpisodeX(_)
                | Token::SeasonOnly(_)
                | Token::EpNumber(_)
        ) {
            // Title is empty - episode marker starts the input
            return;
        }

        // Check for daily format: Number-Hyphen-Number at start (e.g., "11-02 Title")
        // Only apply if no explicit S##E## tokens exist and episode has 2 digits
        if let Token::Number(num_text) = token {
            let token_idx = tokens
                .iter()
                .position(|(_, s)| s.start == span.start && s.end == span.end);
            if let Some(idx) = token_idx {
                // Don't treat as daily format if there's an explicit S##E## token later
                let has_explicit_episode = tokens
                    .iter()
                    .any(|(t, _)| matches!(t, Token::SeasonEpisode(_) | Token::SeasonEpisodeX(_)));

                if !has_explicit_episode && idx + 2 < tokens.len() {
                    if matches!(tokens[idx + 1].0, Token::Hyphen) {
                        if let Token::Number(ep_text) = tokens[idx + 2].0 {
                            // Episode must be exactly 2 digits (e.g., "02" not "7")
                            if ep_text.len() == 2 {
                                // Validate as daily format (both numbers 1-99)
                                if let (Ok(season), Ok(episode)) =
                                    (num_text.parse::<u16>(), ep_text.parse::<u16>())
                                {
                                    if season >= 1 && season <= 99 && episode >= 1 && episode <= 99
                                    {
                                        // Daily format at start - title is empty
                                        return;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Find the first "stop token" that indicates title end
    // Year tokens need special handling - only stop if it's the release year AND not inside parentheses
    let mut title_end_idx = None;

    for (i, (token, _)) in tokens.iter().enumerate() {
        // Special case: don't stop at years inside parentheses or brackets when IncludeInTitle mode
        // e.g., "Series (2009) - [06x16]" should include "(2009)" in title for Sonarr
        // e.g., "Series [2022] [S25E13]" should include "[2022]" in title
        // But in TreatAsMetadata mode, year is metadata and we should stop at it
        if let Token::Year(_) = token {
            if config.year_in_title == YearInTitleMode::IncludeInTitle {
                // Check if this year is inside parentheses
                let inside_parens = i > 0
                    && tokens
                        .get(i.saturating_sub(1))
                        .map(|(t, _)| matches!(t, Token::ParenOpen))
                        .unwrap_or(false);
                if inside_parens {
                    continue; // Don't treat as stop token - include in title
                }

                // Check if this year is inside square brackets followed by episode markers
                // e.g., "[2022] [S25E13]" -> [2022] should be included in title
                let inside_brackets = i > 0
                    && tokens
                        .get(i.saturating_sub(1))
                        .map(|(t, _)| matches!(t, Token::BracketOpen))
                        .unwrap_or(false);
                if inside_brackets {
                    // Check if followed by bracket close and then episode marker in another bracket
                    let followed_by_episode = i + 3 < tokens.len()
                        && matches!(tokens[i + 1].0, Token::BracketClose)
                        && matches!(tokens[i + 2].0, Token::BracketOpen)
                        && tokens[i + 3..].iter().any(|(t, _)| {
                            matches!(t, Token::SeasonEpisode(_) | Token::SeasonEpisodeX(_))
                        });
                    if followed_by_episode {
                        continue; // Don't treat as stop token - include in title
                    }
                }
            }
        }

        // Special case: "Part" followed by a number or word-number (e.g., "Part.Two", "Part 1")
        // should stop the title, BUT only if followed by TV metadata (quality, source), NOT a year
        // Movies like "Dune Part Two 2024" should include "Part Two" in the title
        if let Token::Word(text) = token {
            if text.eq_ignore_ascii_case("PART") {
                // Look ahead for the next non-delimiter token
                let remaining: Vec<_> = tokens
                    .iter()
                    .skip(i + 1)
                    .filter(|(t, _)| !matches!(t, Token::Dot | Token::Hyphen | Token::Underscore))
                    .take(3) // Look at next 3 content tokens
                    .collect();

                if let Some((next_token, _)) = remaining.first() {
                    let is_episode_marker = match next_token {
                        Token::Number(_) => true,
                        Token::Word(w) => is_word_number(w),
                        _ => false,
                    };

                    // Check if "Part X" is followed by a year - if so, it's a movie title, not episode marker
                    // e.g., "Dune.Part.Two.2024" - the 2024 indicates movie
                    // e.g., "Show.Part.Two.720p" - no year after, so it's TV episode
                    let followed_by_year = remaining
                        .get(1)
                        .map_or(false, |(t, _)| matches!(t, Token::Year(_)));

                    if is_episode_marker && !followed_by_year {
                        title_end_idx = Some(i);
                        break;
                    }
                }
            }
        }

        if is_stop_token(token, release) {
            title_end_idx = Some(i);
            break;
        }
    }

    // If no stop token found, use all non-bracket tokens
    let end_idx = title_end_idx.unwrap_or(tokens.len());

    // Collect title tokens
    let mut title_parts = Vec::new();
    let mut first_span = None;
    let mut last_span = None;
    let mut prev_end = 0;

    // Check if we have episodes detected (to filter them from title)
    let has_episodes = !release.seasons.is_empty()
        || !release.episodes.is_empty()
        || release.absolute_episode.is_some();

    // Track indices to skip (for parentheses content we've already processed)
    let mut skip_indices: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for (i, (token, span)) in tokens.iter().enumerate().take(end_idx) {
        // Skip if we've already processed this token (e.g., year inside parens)
        if skip_indices.contains(&i) {
            continue;
        }

        // Handle square brackets specially - include [YEAR] in title when followed by episode markers
        // But only in IncludeInTitle mode (Sonarr style)
        if matches!(token, Token::BracketOpen) {
            if config.year_in_title == YearInTitleMode::IncludeInTitle && i + 2 < end_idx {
                if let Some((Token::Year(year_text), _)) = tokens.get(i + 1) {
                    if let Some((Token::BracketClose, _)) = tokens.get(i + 2) {
                        // Check if followed by episode marker (more bracket content with S##E##)
                        let followed_by_episode = (i + 4 < tokens.len())
                            && matches!(tokens[i + 3].0, Token::BracketOpen)
                            && tokens[i + 4..].iter().any(|(t, _)| {
                                matches!(
                                    t,
                                    Token::SeasonEpisode(_)
                                        | Token::SeasonEpisodeX(_)
                                        | Token::SeasonOnly(_)
                                )
                            });

                        if followed_by_episode {
                            // Found "[YEAR]" followed by episode - include in title
                            if !title_parts.is_empty() {
                                title_parts.push(" ".to_string());
                            }
                            title_parts.push("[".to_string());
                            title_parts.push(year_text.to_string());
                            title_parts.push("]".to_string());
                            // Mark these tokens as processed
                            skip_indices.insert(i + 1);
                            skip_indices.insert(i + 2);
                            prev_end = tokens[i + 2].1.end;
                            if first_span.is_none() {
                                first_span = Some(span.clone());
                            }
                            last_span = Some(tokens[i + 2].1.clone());
                            continue;
                        }
                    }
                }
            }
            // Otherwise skip the bracket
            continue;
        }
        if matches!(token, Token::BracketClose) {
            continue;
        }

        // Phase 3: Handle parentheses specially - include parenthesized years in title
        // But only in IncludeInTitle mode (Sonarr style)
        if matches!(token, Token::ParenOpen) {
            // Check if this paren contains a year that should be in the title
            // Pattern: "(2009)" before episode markers - only include in IncludeInTitle mode
            if config.year_in_title == YearInTitleMode::IncludeInTitle && i + 2 < end_idx {
                if let Some((Token::Year(year_text), _)) = tokens.get(i + 1) {
                    if let Some((Token::ParenClose, _)) = tokens.get(i + 2) {
                        // Found "(YEAR)" pattern - include in title
                        if !title_parts.is_empty() {
                            title_parts.push(" ".to_string());
                        }
                        title_parts.push("(".to_string());
                        title_parts.push(year_text.to_string());
                        title_parts.push(")".to_string());
                        // Mark these tokens as processed
                        skip_indices.insert(i + 1);
                        skip_indices.insert(i + 2);
                        prev_end = tokens[i + 2].1.end;
                        if first_span.is_none() {
                            first_span = Some(span.clone());
                        }
                        last_span = Some(tokens[i + 2].1.clone());
                        continue;
                    }
                }
            }
            // Otherwise skip the paren
            continue;
        }

        if matches!(token, Token::ParenClose) {
            continue;
        }

        // Skip delimiters at the start
        if title_parts.is_empty() && matches!(token, Token::Dot | Token::Hyphen | Token::Underscore)
        {
            continue;
        }

        // For anime-style releases, skip the first bracketed group (release group)
        // This is handled by checking if we're at the start and inside brackets
        if i < 10 && is_inside_brackets(tokens, i) {
            continue;
        }

        // Skip tokens that match the release group content
        // Only skip if the token is inside brackets at the start (anime-style groups)
        if let Some(ref group) = release.release_group {
            if let Token::Word(word) = token {
                // Only skip if we're inside brackets near the start
                if i < 10 && is_inside_brackets(tokens, i) {
                    // Check if this word is part of the release group
                    if group.value.to_lowercase().contains(&word.to_lowercase()) {
                        // Skip this token as it's part of the release group
                        continue;
                    }
                }
            }
        }

        // Check if we need to add a space before this token
        // (if there was a gap in the input between previous and current token)
        let needs_space = !title_parts.is_empty() && prev_end > 0 && span.start > prev_end;

        if first_span.is_none() {
            first_span = Some(span.clone());
        }
        last_span = Some(span.clone());

        match token {
            Token::Word(text) => {
                // Skip "Part" when it's followed by a number that is an episode
                if text.to_uppercase() == "PART" && has_episodes {
                    // Check if followed by an episode number
                    if i + 1 < tokens.len() {
                        if let Token::Number(num_text) = tokens[i + 1].0 {
                            if let Ok(num) = num_text.parse::<u16>() {
                                if release.episodes.iter().any(|ep| ep.value == num) {
                                    // Skip "Part" as it's part of episode marker
                                    continue;
                                }
                            }
                        }
                    }
                }

                // Check for embedded S##E## pattern (e.g., "zoos01e11")
                // Include only the prefix in the title, then stop
                if let Some((prefix, _, _)) = extract_embedded_episode(text) {
                    if !prefix.is_empty() {
                        if needs_space {
                            title_parts.push(" ".to_string());
                        }
                        title_parts.push(prefix.to_string());
                        prev_end = span.start + prefix.len();
                        last_span = Some(std::ops::Range {
                            start: span.start,
                            end: span.start + prefix.len(),
                        });
                    }
                    // Stop collecting title - the embedded episode marks the end
                    break;
                }

                if needs_space {
                    title_parts.push(" ".to_string());
                }
                title_parts.push(text.to_string());
                // Check if there's punctuation immediately after this word in the original input
                if span.end < input.len() {
                    let next_char = input.as_bytes()[span.end] as char;
                    if matches!(next_char, '!' | '?' | '\'' | ';' | ':') {
                        title_parts.push(next_char.to_string());
                        prev_end = span.end + 1;
                    } else {
                        prev_end = span.end;
                    }
                } else {
                    prev_end = span.end;
                }
            }
            Token::Year(text) => {
                // Only skip if this is the actual release year
                if let Ok(num) = text.parse::<u16>() {
                    let is_release_year = release
                        .year
                        .as_ref()
                        .map(|y| y.value == num)
                        .unwrap_or(false);

                    if !is_release_year {
                        // This year is part of the title (e.g., "2049" in "Blade Runner 2049")
                        if needs_space {
                            title_parts.push(" ".to_string());
                        }
                        title_parts.push(text.to_string());
                        prev_end = span.end;
                    }
                }
            }
            Token::Number(text) => {
                // Use u32 to handle larger numbers like 100000
                if let Ok(num) = text.parse::<u32>() {
                    // Skip if this number is the source of a compressed episode
                    if has_episodes {
                        // Check if this is a compressed episode source (e.g., "103" for S01E03)
                        // The raw field on seasons/episodes tells us what was parsed
                        // Only skip if this specific token was used to detect the episode
                        let is_compressed_source = release.seasons.iter().any(|s| s.raw == *text)
                            || release.episodes.iter().any(|e| e.raw == *text);

                        // Also skip if this is an absolute episode that was detected
                        // (but only if it matches the absolute episode value, not regular episodes)
                        let is_absolute_source = release
                            .absolute_episode
                            .as_ref()
                            .map(|ep| ep.raw == *text && ep.value as u32 == num)
                            .unwrap_or(false);

                        if is_compressed_source || is_absolute_source {
                            continue;
                        }

                        if needs_space {
                            title_parts.push(" ".to_string());
                        }
                        title_parts.push(text.to_string());
                        prev_end = span.end;
                    } else {
                        // Check if this looks like a compressed episode number (3-4 digits)
                        // Numbers like 103, 113, 1013, 0308 look like compressed episodes
                        if text.len() == 3 && num >= 100 && num <= 999 {
                            let season = num / 100;
                            let episode = num % 100;
                            if season >= 1 && season <= 9 && episode >= 1 && episode <= 99 {
                                // This is a compressed episode, stop here
                                break;
                            }
                        } else if text.len() == 4 {
                            // Parse first two chars as season, last two as episode
                            // This handles both "1013" (S10E13) and "0308" (S03E08)
                            if let (Ok(season), Ok(episode)) =
                                (text[0..2].parse::<u16>(), text[2..4].parse::<u16>())
                            {
                                if season >= 1 && season <= 99 && episode >= 1 && episode <= 99 {
                                    // This is a compressed episode, stop here
                                    break;
                                }
                            }
                        }

                        // Include all other numbers in title (including large ones like 100000)
                        if needs_space {
                            title_parts.push(" ".to_string());
                        }
                        title_parts.push(text.to_string());
                        prev_end = span.end;
                    }
                }
            }
            Token::Dot | Token::Underscore => {
                // Convert delimiters to spaces
                if !title_parts.is_empty() {
                    title_parts.push(" ".to_string());
                }
                prev_end = span.end;
            }
            Token::Hyphen => {
                // Hyphens can be part of compound words or delimiters
                // Check if the next token is an absolute episode number
                if i + 1 < tokens.len() {
                    if let (Token::Number(next_text), _) = &tokens[i + 1] {
                        if let Ok(num) = next_text.parse::<u16>() {
                            if release
                                .absolute_episode
                                .as_ref()
                                .map(|ep| ep.value == num)
                                .unwrap_or(false)
                            {
                                // This hyphen precedes an absolute episode, stop here
                                break;
                            }
                        }
                    }
                }

                // Phase 2: Check if hyphen connects word-to-number (preserve) vs word-to-word (space)
                // Patterns like "Title-0", "24-7" should preserve the hyphen
                let prev_is_word_or_number = i > 0
                    && matches!(
                        tokens.get(i - 1).map(|(t, _)| t),
                        Some(Token::Word(_)) | Some(Token::Number(_))
                    );
                let next_is_number = i + 1 < tokens.len()
                    && matches!(tokens.get(i + 1).map(|(t, _)| t), Some(Token::Number(_)));

                if prev_is_word_or_number && next_is_number && !title_parts.is_empty() {
                    // Preserve hyphen in patterns like "Title-0", "24-7"
                    title_parts.push("-".to_string());
                } else if !title_parts.is_empty() {
                    // Otherwise, treat as space
                    title_parts.push(" ".to_string());
                }
                prev_end = span.end;
            }
            Token::Ampersand => {
                // Preserve ampersand in title
                if needs_space {
                    title_parts.push(" ".to_string());
                }
                title_parts.push("&".to_string());
                prev_end = span.end;
            }
            _ => {}
        }
    }

    // Clean up and normalize title
    let title = title_parts.join("").trim().to_string();

    // Normalize multiple spaces
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");

    if !title.is_empty() {
        let (span, raw_text) = if let (Some(ref first), Some(ref last)) = (first_span, last_span) {
            ((first.start, last.end), &input[first.start..last.end])
        } else {
            ((0, 0), "")
        };

        release.title = crate::model::ParsedField::new(title, Confidence::HIGH, span, raw_text);
    }
}

/// Check if a token indicates the end of the title.
fn is_stop_token(token: &Token, release: &ParsedRelease) -> bool {
    match token {
        Token::Year(text) => {
            // Only treat as stop token if this is the release year
            if let Ok(year) = text.parse::<u16>() {
                release
                    .year
                    .as_ref()
                    .map(|y| y.value == year)
                    .unwrap_or(false)
            } else {
                false
            }
        }
        Token::Number(text) => {
            // Stop at numbers that are detected absolute episodes
            if let Ok(num) = text.parse::<u16>() {
                let is_absolute_ep = release
                    .absolute_episode
                    .as_ref()
                    .map(|ep| ep.value == num)
                    .unwrap_or(false);

                // Also check if this is a compressed episode source (e.g., "209" for S02E09, "0308" for S03E08)
                // The raw field contains the original text (3 or 4 digits)
                let is_compressed_source = release
                    .episodes
                    .iter()
                    .any(|ep| ep.raw == *text && (text.len() == 3 || text.len() == 4));

                is_absolute_ep || is_compressed_source
            } else {
                false
            }
        }
        Token::Word(text) => {
            // Stop at certain keywords that indicate metadata
            let upper = text.to_uppercase();
            matches!(
                upper.as_str(),
                "COMPLETE" | "REMASTERED" | "PROPER" | "REPACK" | "TEMPORADA" | "SE" | "AFL"
            )
            // Note: Words with embedded S##E## (like "zoos01e11") are NOT stop tokens
            // because their prefix should be included in the title - this is handled
            // during title collection where we extract only the prefix portion
        }
        Token::SeasonEpisode(_)
        | Token::SeasonEpisodeX(_)
        | Token::SeasonOnly(_)
        | Token::SeasonWord(_)
        | Token::EpisodeWord(_)
        | Token::EpNumber(_)
        | Token::PartEpisode(_)
        | Token::EpisodeOfTotal(_)
        | Token::Resolution(_)
        | Token::SourceBluray(_)
        | Token::SourceBdRip(_)
        | Token::SourceWebDL(_)
        | Token::SourceWebRip(_)
        | Token::SourceHD(_)
        | Token::SourceDVD(_)
        | Token::CodecH264(_)
        | Token::CodecH265(_)
        | Token::CodecAv1(_)
        | Token::StreamingService(_) => true,
        _ => false,
    }
}

/// Check if a word is a spelled-out number (One, Two, Three, etc.)
fn is_word_number(word: &str) -> bool {
    matches!(
        word.to_uppercase().as_str(),
        "ONE"
            | "I"
            | "TWO"
            | "II"
            | "THREE"
            | "III"
            | "FOUR"
            | "IV"
            | "FIVE"
            | "V"
            | "SIX"
            | "VI"
            | "SEVEN"
            | "VII"
            | "EIGHT"
            | "VIII"
            | "NINE"
            | "IX"
            | "TEN"
            | "X"
            | "ELEVEN"
            | "XI"
            | "TWELVE"
            | "XII"
    )
}

/// Check if a token at the given index is inside square brackets `[]`.
/// This is used to skip anime-style release groups like `[SubGroup]`.
/// Note: Does NOT check for parentheses `()` - those can contain valid title content like years.
fn is_inside_brackets(tokens: &[(Token, std::ops::Range<usize>)], idx: usize) -> bool {
    // Look backwards for an opening square bracket (not parentheses)
    let mut depth = 0;
    for i in (0..idx).rev() {
        match &tokens[i].0 {
            Token::BracketClose => depth += 1,
            Token::BracketOpen => {
                depth -= 1;
                if depth < 0 {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_title() {
        let input = "The.Matrix.1999.1080p.BluRay.x264-GROUP";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        // Run other parsers first
        crate::parser::episode::extract(&lexer, &mut release);
        crate::parser::quality::extract(&lexer, &mut release);
        crate::parser::codec::extract(&lexer, &mut release);
        crate::parser::metadata::extract(&lexer, &mut release);

        extract(&lexer, &mut release);

        assert_eq!(*release.title, "The Matrix");
    }

    #[test]
    fn test_extract_tv_title() {
        let input = "Breaking.Bad.S01E01.720p.BluRay.x264-DEMAND";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        crate::parser::episode::extract(&lexer, &mut release);
        crate::parser::quality::extract(&lexer, &mut release);
        crate::parser::codec::extract(&lexer, &mut release);
        crate::parser::metadata::extract(&lexer, &mut release);

        extract(&lexer, &mut release);

        assert_eq!(*release.title, "Breaking Bad");
    }

    #[test]
    fn test_extract_anime_title() {
        let input = "[SubGroup] Anime Title - 01 [1080p].mkv";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        crate::parser::episode::extract(&lexer, &mut release);
        crate::parser::quality::extract(&lexer, &mut release);
        crate::parser::codec::extract(&lexer, &mut release);
        crate::parser::metadata::extract(&lexer, &mut release);

        extract(&lexer, &mut release);

        // Should extract "Anime Title" without the release group
        assert!(release.title.to_string().contains("Anime Title"));
    }

    #[test]
    fn test_extract_title_with_year() {
        // When year is immediately followed by season/episode, it's part of the title
        // This matches Sonarr's expectation for "Series Title 2010 S02E14"
        let input = "Shogun.2024.S01E10.720p.HDTV";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        crate::parser::episode::extract(&lexer, &mut release);
        crate::parser::quality::extract(&lexer, &mut release);
        crate::parser::codec::extract(&lexer, &mut release);
        crate::parser::metadata::extract(&lexer, &mut release);

        extract(&lexer, &mut release);

        assert_eq!(*release.title, "Shogun 2024");
    }

    #[test]
    fn test_extract_bracket_year_in_title() {
        // [2022] should be included in title when followed by [S##E##]
        let input = "Series Title [2022] [S25E13] [PL] [720p] [WEB-DL-CZRG] [x264]";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        crate::parser::episode::extract(&lexer, &mut release);
        crate::parser::quality::extract(&lexer, &mut release);
        crate::parser::codec::extract(&lexer, &mut release);
        crate::parser::metadata::extract(&lexer, &mut release);

        extract_with_config(&lexer, &mut release, &ParserConfig::default());

        assert_eq!(*release.title, "Series Title [2022]");
    }
}
