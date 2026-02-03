//! Episode metadata parser.
//!
//! Extracts season numbers, episode numbers, absolute episodes,
//! and determines media type (TV vs Movie).

use crate::lexer::{Lexer, Token};
use crate::model::{Confidence, MediaType, ParsedField, ParsedRelease};

/// Convert a word number (One, Two, etc.) to its numeric value.
/// Returns None if the word is not a recognized number word.
fn word_to_number(word: &str) -> Option<u16> {
    match word.to_uppercase().as_str() {
        "ONE" | "I" => Some(1),
        "TWO" | "II" => Some(2),
        "THREE" | "III" => Some(3),
        "FOUR" | "IV" => Some(4),
        "FIVE" | "V" => Some(5),
        "SIX" | "VI" => Some(6),
        "SEVEN" | "VII" => Some(7),
        "EIGHT" | "VIII" => Some(8),
        "NINE" | "IX" => Some(9),
        "TEN" | "X" => Some(10),
        "ELEVEN" | "XI" => Some(11),
        "TWELVE" | "XII" => Some(12),
        _ => None,
    }
}

/// Extract episode information from the token stream.
///
/// This parser looks for season/episode patterns in the tokens and populates
/// the `seasons`, `episodes`, `absolute_episode`, and `media_type` fields.
pub fn extract(lexer: &Lexer, release: &mut ParsedRelease) {
    let tokens = lexer.tokens();

    for (token, span) in tokens {
        match token {
            Token::SeasonEpisode(text) | Token::SeasonEpisodeX(text) => {
                if let Some((seasons, mut episodes)) = parse_season_episode(text) {
                    // Check for episode range: SeasonEpisode - Hyphen - Number/E##
                    let token_idx = tokens
                        .iter()
                        .position(|(_, s)| s.start == span.start && s.end == span.end);

                    if let Some(idx) = token_idx {
                        // Check if followed by: Hyphen, then E## or just ##
                        // Handle chained ranges like S02E03-04-05
                        let mut current_idx = idx;
                        while current_idx + 2 < tokens.len() {
                            if let Token::Hyphen = tokens[current_idx + 1].0 {
                                // Check what follows the hyphen
                                match &tokens[current_idx + 2].0 {
                                    Token::SeasonEpisode(range_text) => {
                                        // S01E01-E05 format (E05 is parsed as SeasonEpisode)
                                        if let Some((_, range_eps)) =
                                            parse_season_episode(range_text)
                                        {
                                            if let (Some(&start), Some(&end)) =
                                                (episodes.last(), range_eps.first())
                                            {
                                                // Expand the range
                                                for ep in start..=end {
                                                    if !episodes.contains(&ep) {
                                                        episodes.push(ep);
                                                    }
                                                }
                                            }
                                        }
                                        current_idx += 2; // Move past hyphen and E## to check for more
                                    }
                                    Token::Number(num_text) => {
                                        // S01E01-05 format (just the number)
                                        if let Ok(end_ep) = num_text.parse::<u16>() {
                                            // Check if the number token is immediately followed by
                                            // a delimiter, hyphen (for chaining), or quality token
                                            let is_range = if current_idx + 3 < tokens.len() {
                                                matches!(
                                                    tokens[current_idx + 3].0,
                                                    Token::Dot
                                                        | Token::Hyphen
                                                        | Token::Resolution(_)
                                                        | Token::SourceBluray(_)
                                                        | Token::SourceWebDL(_)
                                                        | Token::SourceHD(_)
                                                        | Token::CodecH264(_)
                                                        | Token::CodecH265(_)
                                                )
                                            } else {
                                                true // At end of tokens
                                            };

                                            if is_range {
                                                if let Some(&start) = episodes.last() {
                                                    // Sanity check: end shouldn't be too far from start
                                                    if (start..=start + 50).contains(&end_ep) {
                                                        for ep in start..=end_ep {
                                                            if !episodes.contains(&ep) {
                                                                episodes.push(ep);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        current_idx += 2; // Move past hyphen and number to check for more
                                    }
                                    Token::Word(word_text) => {
                                        // S01E01-E05 where E05 is matched as Word
                                        if let Some(end_ep) = parse_dot_episode(word_text) {
                                            if let Some(&start) = episodes.last() {
                                                for ep in start..=end_ep {
                                                    if !episodes.contains(&ep) {
                                                        episodes.push(ep);
                                                    }
                                                }
                                            }
                                        }
                                        current_idx += 2;
                                    }
                                    _ => break, // No more range continuation
                                }
                            } else {
                                break; // Not a hyphen, stop looking for ranges
                            }
                        }
                    }

                    // Add seasons
                    for season in seasons {
                        if !release.seasons.iter().any(|s| **s == season) {
                            release.seasons.push(ParsedField::new(
                                season,
                                Confidence::CERTAIN,
                                (span.start, span.end),
                                *text,
                            ));
                        }
                    }

                    // Add episodes
                    for episode in episodes {
                        if !release.episodes.iter().any(|e| **e == episode) {
                            release.episodes.push(ParsedField::new(
                                episode,
                                Confidence::CERTAIN,
                                (span.start, span.end),
                                *text,
                            ));
                        }
                    }

                    // If we found episodes, this is definitely TV
                    release.media_type = ParsedField::new(
                        MediaType::Tv,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    );
                }
            }
            Token::SeasonOnly(text) => {
                // Season-only token (e.g., "S01" without episode) - might be full season release
                // But first check if followed by dot/hyphen/space + episode (s03.e05, S1-E1, S15 E06)
                if let Some(season) = parse_season_only(text) {
                    // Look for dot/hyphen/space-separated episode format
                    let token_idx = tokens
                        .iter()
                        .position(|(_, s)| s.start == span.start && s.end == span.end);

                    if let Some(idx) = token_idx {
                        // Check for formats: s##.e##, S##-E##, S## E##
                        let mut found_episode = false;

                        if idx + 2 < tokens.len() {
                            let delimiter = &tokens[idx + 1].0;
                            // Check if followed by Dot, Hyphen, or if next token is directly an E## Word
                            // (space-separated case where whitespace is skipped)
                            if matches!(delimiter, Token::Dot | Token::Hyphen | Token::Underscore) {
                                // Check for s06.01 format (SeasonOnly + Dot + Number)
                                if let Token::Number(num_text) = tokens[idx + 2].0 {
                                    if let Ok(episode) = num_text.parse::<u16>() {
                                        // Make sure it's a reasonable episode number (1-999)
                                        if (1..=999).contains(&episode) {
                                            // Found s06.01 format
                                            if !release.seasons.iter().any(|s| **s == season) {
                                                release.seasons.push(ParsedField::new(
                                                    season,
                                                    Confidence::CERTAIN,
                                                    (span.start, span.end),
                                                    *text,
                                                ));
                                            }
                                            if !release.episodes.iter().any(|e| **e == episode) {
                                                release.episodes.push(ParsedField::new(
                                                    episode,
                                                    Confidence::CERTAIN,
                                                    (
                                                        tokens[idx + 2].1.start,
                                                        tokens[idx + 2].1.end,
                                                    ),
                                                    num_text,
                                                ));
                                            }
                                            release.media_type = ParsedField::new(
                                                MediaType::Tv,
                                                Confidence::CERTAIN,
                                                (span.start, span.end),
                                                *text,
                                            );
                                            found_episode = true;
                                        }
                                    }
                                }
                                if !found_episode {
                                    if let Token::Word(ep_text) = tokens[idx + 2].0 {
                                        // Check if the word is like "e05", "E12", etc.
                                        if let Some(episode) = parse_dot_episode(ep_text) {
                                            // Found s##.e## or S##-E## format
                                            if !release.seasons.iter().any(|s| **s == season) {
                                                release.seasons.push(ParsedField::new(
                                                    season,
                                                    Confidence::CERTAIN,
                                                    (span.start, span.end),
                                                    *text,
                                                ));
                                            }
                                            if !release.episodes.iter().any(|e| **e == episode) {
                                                release.episodes.push(ParsedField::new(
                                                    episode,
                                                    Confidence::CERTAIN,
                                                    (
                                                        tokens[idx + 2].1.start,
                                                        tokens[idx + 2].1.end,
                                                    ),
                                                    ep_text,
                                                ));
                                            }
                                            release.media_type = ParsedField::new(
                                                MediaType::Tv,
                                                Confidence::CERTAIN,
                                                (span.start, span.end),
                                                *text,
                                            );
                                            found_episode = true;
                                        }
                                        // Check for S01.Ep.01 or S01.E.01 pattern (Ep/E followed by dot and number)
                                        else if idx + 4 < tokens.len() {
                                            let ep_upper = ep_text.to_uppercase();
                                            if (ep_upper == "EP" || ep_upper == "E")
                                                && matches!(
                                                    tokens[idx + 3].0,
                                                    Token::Dot | Token::Hyphen | Token::Underscore
                                                )
                                            {
                                                if let Token::Number(num_text) =
                                                    tokens[idx + 4].0
                                                {
                                                    if let Ok(episode) = num_text.parse::<u16>()
                                                    {
                                                        // Found S01.Ep.01 or S01.E.01 format
                                                        if !release
                                                            .seasons
                                                            .iter()
                                                            .any(|s| **s == season)
                                                        {
                                                            release.seasons.push(
                                                                ParsedField::new(
                                                                    season,
                                                                    Confidence::CERTAIN,
                                                                    (span.start, span.end),
                                                                    *text,
                                                                ),
                                                            );
                                                        }
                                                        if !release
                                                            .episodes
                                                            .iter()
                                                            .any(|e| **e == episode)
                                                        {
                                                            release.episodes.push(
                                                                ParsedField::new(
                                                                    episode,
                                                                    Confidence::CERTAIN,
                                                                    (
                                                                        tokens[idx + 4].1.start,
                                                                        tokens[idx + 4].1.end,
                                                                    ),
                                                                    num_text,
                                                                ),
                                                            );
                                                        }
                                                        release.media_type = ParsedField::new(
                                                            MediaType::Tv,
                                                            Confidence::CERTAIN,
                                                            (span.start, span.end),
                                                            *text,
                                                        );
                                                        found_episode = true;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Check for space-separated: S## E## (whitespace is skipped, so E## is idx+1)
                        if !found_episode && idx + 1 < tokens.len() {
                            if let Token::Word(ep_text) = tokens[idx + 1].0 {
                                // Check if the word is like "E05", "E12", etc.
                                if let Some(episode) = parse_dot_episode(ep_text) {
                                    // Found S## E## format (space-separated)
                                    if !release.seasons.iter().any(|s| **s == season) {
                                        release.seasons.push(ParsedField::new(
                                            season,
                                            Confidence::CERTAIN,
                                            (span.start, span.end),
                                            *text,
                                        ));
                                    }
                                    if !release.episodes.iter().any(|e| **e == episode) {
                                        release.episodes.push(ParsedField::new(
                                            episode,
                                            Confidence::CERTAIN,
                                            (tokens[idx + 1].1.start, tokens[idx + 1].1.end),
                                            ep_text,
                                        ));
                                    }
                                    release.media_type = ParsedField::new(
                                        MediaType::Tv,
                                        Confidence::CERTAIN,
                                        (span.start, span.end),
                                        *text,
                                    );
                                    found_episode = true;
                                }
                            }
                        }

                        if found_episode {
                            continue; // Don't mark as full season
                        }
                    }

                    // No episode found - treat as full season release
                    if !release.seasons.iter().any(|s| **s == season) {
                        release.seasons.push(ParsedField::new(
                            season,
                            Confidence::CERTAIN,
                            (span.start, span.end),
                            *text,
                        ));
                    }
                    // Mark as full season release
                    release.full_season = true;
                    // This is definitely TV
                    release.media_type = ParsedField::new(
                        MediaType::Tv,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    );
                }
            }
            Token::PartEpisode(text) => {
                // Part episode format (e.g., "Part01", "Part1", "Part02")
                if let Some(episode) = parse_part_episode(text) {
                    // Part episodes are typically season 1
                    if release.seasons.is_empty() {
                        release.seasons.push(ParsedField::new(
                            1,
                            Confidence::HIGH,
                            (span.start, span.end),
                            *text,
                        ));
                    }
                    if !release.episodes.iter().any(|e| **e == episode) {
                        release.episodes.push(ParsedField::new(
                            episode,
                            Confidence::CERTAIN,
                            (span.start, span.end),
                            *text,
                        ));
                    }
                    // This is TV
                    release.media_type = ParsedField::new(
                        MediaType::Tv,
                        Confidence::HIGH,
                        (span.start, span.end),
                        *text,
                    );
                }
            }
            Token::SeasonWord(_) => {
                // Spelled-out "Season" followed by number (e.g., "Season 01", "Season 1")
                let token_idx = tokens
                    .iter()
                    .position(|(_, s)| s.start == span.start && s.end == span.end);

                if let Some(idx) = token_idx {
                    if idx + 1 < tokens.len() {
                        if let Token::Number(num_text) = tokens[idx + 1].0 {
                            if let Ok(season) = num_text.parse::<u16>() {
                                if !release.seasons.iter().any(|s| **s == season) {
                                    release.seasons.push(ParsedField::new(
                                        season,
                                        Confidence::CERTAIN,
                                        (span.start, tokens[idx + 1].1.end),
                                        lexer
                                            .input()
                                            .get(span.start..tokens[idx + 1].1.end)
                                            .unwrap_or(""),
                                    ));
                                }
                                release.media_type = ParsedField::new(
                                    MediaType::Tv,
                                    Confidence::CERTAIN,
                                    (span.start, span.end),
                                    lexer.input().get(span.start..span.end).unwrap_or(""),
                                );
                            }
                        }
                    }
                }
            }
            Token::EpisodeWord(_) => {
                // Spelled-out "Episode" followed by number (e.g., "Episode 01", "Episode 5")
                let token_idx = tokens
                    .iter()
                    .position(|(_, s)| s.start == span.start && s.end == span.end);

                if let Some(idx) = token_idx {
                    if idx + 1 < tokens.len() {
                        if let Token::Number(num_text) = tokens[idx + 1].0 {
                            if let Ok(episode) = num_text.parse::<u16>() {
                                if !release.episodes.iter().any(|e| **e == episode) {
                                    release.episodes.push(ParsedField::new(
                                        episode,
                                        Confidence::CERTAIN,
                                        (span.start, tokens[idx + 1].1.end),
                                        lexer
                                            .input()
                                            .get(span.start..tokens[idx + 1].1.end)
                                            .unwrap_or(""),
                                    ));
                                }
                                release.media_type = ParsedField::new(
                                    MediaType::Tv,
                                    Confidence::CERTAIN,
                                    (span.start, span.end),
                                    lexer.input().get(span.start..span.end).unwrap_or(""),
                                );
                            }
                        }
                    }
                }
            }
            Token::EpisodeOfTotal(text) => {
                // X of Y format (e.g., "5of9" -> episode 5)
                if let Some(episode) = parse_episode_of_total(text) {
                    // These are typically season 1
                    if release.seasons.is_empty() {
                        release.seasons.push(ParsedField::new(
                            1,
                            Confidence::HIGH,
                            (span.start, span.end),
                            *text,
                        ));
                    }
                    if !release.episodes.iter().any(|e| **e == episode) {
                        release.episodes.push(ParsedField::new(
                            episode,
                            Confidence::CERTAIN,
                            (span.start, span.end),
                            *text,
                        ));
                    }
                    release.media_type = ParsedField::new(
                        MediaType::Tv,
                        Confidence::HIGH,
                        (span.start, span.end),
                        *text,
                    );
                }
            }
            Token::EpNumber(text) => {
                // Abbreviated "Ep06" format -> episode 6
                if let Some(episode) = parse_ep_number(text) {
                    if !release.episodes.iter().any(|e| **e == episode) {
                        release.episodes.push(ParsedField::new(
                            episode,
                            Confidence::CERTAIN,
                            (span.start, span.end),
                            *text,
                        ));
                    }
                    release.media_type = ParsedField::new(
                        MediaType::Tv,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    );
                }
            }
            Token::Word(text) => {
                let upper = text.to_uppercase();

                // Check for embedded S##E## pattern in word (e.g., "zoos01e11" -> S01E11)
                // This handles cases where the episode marker is joined with a prefix word
                if release.seasons.is_empty() && release.episodes.is_empty() {
                    if let Some((prefix, season, episodes)) = extract_embedded_episode(text) {
                        // Store the prefix info for title extraction (via the span we're using)
                        // Note: The prefix becomes part of the title, handled in title.rs
                        release.seasons.push(ParsedField::new(
                            season,
                            Confidence::CERTAIN,
                            (span.start + prefix.len(), span.end),
                            *text,
                        ));
                        for episode in episodes {
                            if !release.episodes.iter().any(|e| **e == episode) {
                                release.episodes.push(ParsedField::new(
                                    episode,
                                    Confidence::CERTAIN,
                                    (span.start + prefix.len(), span.end),
                                    *text,
                                ));
                            }
                        }
                        release.media_type = ParsedField::new(
                            MediaType::Tv,
                            Confidence::HIGH,
                            (span.start + prefix.len(), span.end),
                            "",
                        );
                        continue;
                    }
                }

                // Check for "Part ##" format (Part with space before number)
                // But NOT when "Part" is preceded by a comma (e.g., "Halloween, Part 1" is episode title)
                if upper == "PART" {
                    // Look ahead for a number token
                    let token_idx = tokens
                        .iter()
                        .position(|(_, s)| s.start == span.start && s.end == span.end);

                    if let Some(idx) = token_idx {
                        // Check if preceded by comma in the input (looking at raw input)
                        // This indicates "Part X" is part of the episode title, not episode marker
                        let preceded_by_comma = span.start > 0 && {
                            let before = &lexer.input()[..span.start];
                            before.trim_end().ends_with(',')
                        };

                        // Also check if we already have episodes detected
                        // "Part 1" after an existing episode is probably part of the title
                        let already_has_episodes = !release.episodes.is_empty();

                        if !preceded_by_comma && !already_has_episodes {
                            // Find the next non-delimiter token after "Part"
                            // (skip over dots, hyphens, underscores)
                            let next_content_idx = tokens
                                .iter()
                                .enumerate()
                                .skip(idx + 1)
                                .find(|(_, (t, _))| {
                                    !matches!(t, Token::Dot | Token::Hyphen | Token::Underscore)
                                })
                                .map(|(i, _)| i);

                            if let Some(next_idx) = next_content_idx {
                                // Try to get episode number from either numeric or word form
                                let (episode, ep_span, ep_raw): (Option<u16>, _, _) =
                                    match &tokens[next_idx].0 {
                                        Token::Number(num_text) => (
                                            num_text.parse::<u16>().ok(),
                                            tokens[next_idx].1.clone(),
                                            *num_text,
                                        ),
                                        Token::Word(word_text) => (
                                            word_to_number(word_text),
                                            tokens[next_idx].1.clone(),
                                            *word_text,
                                        ),
                                        _ => (None, span.clone(), ""),
                                    };

                                if let Some(episode) = episode {
                                    // Part episodes are typically season 1
                                    if release.seasons.is_empty() {
                                        release.seasons.push(ParsedField::new(
                                            1,
                                            Confidence::HIGH,
                                            (span.start, span.end),
                                            *text,
                                        ));
                                    }
                                    if !release.episodes.iter().any(|e| **e == episode) {
                                        release.episodes.push(ParsedField::new(
                                            episode,
                                            Confidence::CERTAIN,
                                            (ep_span.start, ep_span.end),
                                            ep_raw,
                                        ));
                                    }
                                    release.media_type = ParsedField::new(
                                        MediaType::Tv,
                                        Confidence::HIGH,
                                        (span.start, span.end),
                                        *text,
                                    );
                                }
                            }
                        }
                    }
                }

                // Spanish "Temporada" (Season) - e.g., "Temporada 2"
                if upper == "TEMPORADA" {
                    let token_idx = tokens
                        .iter()
                        .position(|(_, s)| s.start == span.start && s.end == span.end);

                    if let Some(idx) = token_idx {
                        // Look for following number (skip delimiters)
                        let next_content_idx = tokens
                            .iter()
                            .enumerate()
                            .skip(idx + 1)
                            .find(|(_, (t, _))| {
                                !matches!(t, Token::Dot | Token::Hyphen | Token::Underscore)
                            })
                            .map(|(i, _)| i);

                        if let Some(next_idx) = next_content_idx {
                            if let Token::Number(num_text) = tokens[next_idx].0 {
                                if let Ok(season) = num_text.parse::<u16>() {
                                    if !release.seasons.iter().any(|s| **s == season) {
                                        release.seasons.push(ParsedField::new(
                                            season,
                                            Confidence::HIGH,
                                            (tokens[next_idx].1.start, tokens[next_idx].1.end),
                                            num_text,
                                        ));
                                    }
                                    release.media_type = ParsedField::new(
                                        MediaType::Tv,
                                        Confidence::HIGH,
                                        (span.start, span.end),
                                        *text,
                                    );
                                }
                            }
                        }
                    }
                }

                // Spanish "Cap" (Episode/Chapter) - e.g., "Cap.201" for S02E01
                // Cap.### takes precedence over Temporada for season/episode extraction
                if upper == "CAP" {
                    let token_idx = tokens
                        .iter()
                        .position(|(_, s)| s.start == span.start && s.end == span.end);

                    if let Some(idx) = token_idx {
                        // Look for Dot + Number/Year pattern (Cap.201 or Cap.1901)
                        // Note: 4-digit numbers like 1901 are tokenized as Year, not Number
                        if idx + 2 < tokens.len()
                            && matches!(tokens[idx + 1].0, Token::Dot)
                        {
                            // Get the number text from either Number or Year token
                            let num_text = match tokens[idx + 2].0 {
                                Token::Number(t) => Some(t),
                                Token::Year(t) => Some(t),
                                _ => None,
                            };

                            if let Some(num_text) = num_text {
                                // Parse as compressed episode (201 -> S02E01, 1901 -> S19E01)
                                if let Some((season, episode)) =
                                    parse_compressed_episode(num_text)
                                {
                                    // Cap.### is authoritative - clear any Temporada season and set from Cap
                                    release.seasons.clear();
                                    release.seasons.push(ParsedField::new(
                                        season,
                                        Confidence::CERTAIN,
                                        (tokens[idx + 2].1.start, tokens[idx + 2].1.end),
                                        num_text,
                                    ));
                                    if !release.episodes.iter().any(|e| **e == episode) {
                                        release.episodes.push(ParsedField::new(
                                            episode,
                                            Confidence::CERTAIN,
                                            (tokens[idx + 2].1.start, tokens[idx + 2].1.end),
                                            num_text,
                                        ));
                                    }
                                    release.media_type = ParsedField::new(
                                        MediaType::Tv,
                                        Confidence::HIGH,
                                        (span.start, tokens[idx + 2].1.end),
                                        "",
                                    );
                                }
                            }
                        }
                    }
                }

                // Dutch "Se" (Season abbreviation) - e.g., "Se.3" for Season 3
                if upper == "SE" {
                    let token_idx = tokens
                        .iter()
                        .position(|(_, s)| s.start == span.start && s.end == span.end);

                    if let Some(idx) = token_idx {
                        // Look for Dot + Number pattern (Se.3)
                        if idx + 2 < tokens.len()
                            && matches!(tokens[idx + 1].0, Token::Dot)
                        {
                            if let Token::Number(num_text) = tokens[idx + 2].0 {
                                if let Ok(season) = num_text.parse::<u16>() {
                                    if (1..=99).contains(&season) {
                                        if !release.seasons.iter().any(|s| **s == season) {
                                            release.seasons.push(ParsedField::new(
                                                season,
                                                Confidence::CERTAIN,
                                                (
                                                    tokens[idx + 2].1.start,
                                                    tokens[idx + 2].1.end,
                                                ),
                                                num_text,
                                            ));
                                        }
                                        release.media_type = ParsedField::new(
                                            MediaType::Tv,
                                            Confidence::HIGH,
                                            (span.start, tokens[idx + 2].1.end),
                                            "",
                                        );
                                    }
                                }
                            }
                        }
                    }
                }

                // Dutch "afl" (Episode abbreviation) - e.g., "afl.3" for Episode 3
                // Can also handle ranges like "afl.2-3-4" for episodes [2,3,4]
                if upper == "AFL" {
                    let token_idx = tokens
                        .iter()
                        .position(|(_, s)| s.start == span.start && s.end == span.end);

                    if let Some(idx) = token_idx {
                        // Look for Dot + Number pattern (afl.3)
                        if idx + 2 < tokens.len()
                            && matches!(tokens[idx + 1].0, Token::Dot)
                        {
                            if let Token::Number(num_text) = tokens[idx + 2].0 {
                                if let Ok(episode) = num_text.parse::<u16>() {
                                    if (1..=9999).contains(&episode) {
                                        if !release.episodes.iter().any(|e| **e == episode) {
                                            release.episodes.push(ParsedField::new(
                                                episode,
                                                Confidence::CERTAIN,
                                                (
                                                    tokens[idx + 2].1.start,
                                                    tokens[idx + 2].1.end,
                                                ),
                                                num_text,
                                            ));
                                        }
                                        release.media_type = ParsedField::new(
                                            MediaType::Tv,
                                            Confidence::HIGH,
                                            (span.start, tokens[idx + 2].1.end),
                                            "",
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Token::Number(text) => {
                // Get token index for lookahead
                let token_idx = tokens
                    .iter()
                    .position(|(_, s)| s.start == span.start && s.end == span.end);

                // Check for daily format: Number-Hyphen-Number at input start (e.g., "11-02 Title")
                // This represents season 11, episode 2 in daily show format
                // IMPORTANT: Only match if:
                // - No explicit S##E## tokens exist elsewhere (otherwise this is part of title)
                // - Episode number has exactly 2 digits (e.g., "02" not "7")
                if release.seasons.is_empty() && release.episodes.is_empty() {
                    if let Some(idx) = token_idx {
                        // Only match if this is at or near the start (no word tokens before)
                        let has_words_before = tokens[..idx]
                            .iter()
                            .any(|(t, _)| matches!(t, Token::Word(_)));

                        // Don't treat as daily format if there's an explicit S##E## token later
                        let has_explicit_episode = tokens.iter().any(|(t, _)| {
                            matches!(t, Token::SeasonEpisode(_) | Token::SeasonEpisodeX(_))
                        });

                        if !has_words_before && !has_explicit_episode && idx + 2 < tokens.len()
                            && matches!(tokens[idx + 1].0, Token::Hyphen)
                        {
                                if let Token::Number(ep_text) = tokens[idx + 2].0 {
                                    // Episode must be exactly 2 digits (e.g., "02" not "7")
                                    // This distinguishes "11-02" (daily) from "24-7" (title)
                                    if ep_text.len() == 2 {
                                        if let (Ok(season), Ok(episode)) =
                                            (text.parse::<u16>(), ep_text.parse::<u16>())
                                        {
                                            // Sanity check: reasonable season/episode values
                                            if (1..=99).contains(&season)
                                                && (1..=99).contains(&episode)
                                            {
                                                // Verify there's content after (not just "11-02")
                                                let has_content_after = tokens
                                                    .iter()
                                                    .skip(idx + 3)
                                                    .any(|(t, _)| matches!(t, Token::Word(_)));

                                                if has_content_after || tokens.len() == 3 {
                                                    release.seasons.push(ParsedField::new(
                                                        season,
                                                        Confidence::HIGH,
                                                        (span.start, tokens[idx + 2].1.end),
                                                        &lexer.input()
                                                            [span.start..tokens[idx + 2].1.end],
                                                    ));
                                                    release.episodes.push(ParsedField::new(
                                                        episode,
                                                        Confidence::HIGH,
                                                        (
                                                            tokens[idx + 2].1.start,
                                                            tokens[idx + 2].1.end,
                                                        ),
                                                        ep_text,
                                                    ));
                                                    release.media_type = ParsedField::new(
                                                        MediaType::Tv,
                                                        Confidence::HIGH,
                                                        (span.start, tokens[idx + 2].1.end),
                                                        "",
                                                    );
                                                    continue;
                                                }
                                            }
                                        }
                                    }
                                }
                        }
                    }
                }

                // Check if preceded by hyphen AND codec token (allows "x264-103")
                // This is checked FIRST and OUTSIDE the empty check, because codec-preceded
                // compressed episodes should override any mistakenly detected absolute episodes
                let preceded_by_codec = if let Some(idx) = token_idx {
                    idx >= 2
                        && matches!(tokens[idx - 1].0, Token::Hyphen)
                        && matches!(
                            tokens[idx - 2].0,
                            Token::CodecH264(_) | Token::CodecH265(_) | Token::CodecAv1(_)
                        )
                } else {
                    false
                };

                // Don't parse as compressed episode if there's a real S##E## token elsewhere
                let has_explicit_episode = tokens
                    .iter()
                    .any(|(t, _)| matches!(t, Token::SeasonEpisode(_) | Token::SeasonEpisodeX(_)));

                // Codec-preceded compressed episode (e.g., "x264-103") - authoritative, overrides others
                if preceded_by_codec && !has_explicit_episode {
                    if let Some((season, episode)) = parse_compressed_episode(text) {
                        // Clear any mistakenly detected episodes (like "7" from "7p")
                        release.seasons.clear();
                        release.episodes.clear();
                        release.absolute_episode = None;
                        // Add season
                        release.seasons.push(ParsedField::new(
                            season,
                            Confidence::HIGH,
                            (span.start, span.end),
                            *text,
                        ));
                        // Add episode
                        release.episodes.push(ParsedField::new(
                            episode,
                            Confidence::HIGH,
                            (span.start, span.end),
                            *text,
                        ));
                        release.media_type = ParsedField::new(
                            MediaType::Tv,
                            Confidence::HIGH,
                            (span.start, span.end),
                            *text,
                        );
                        continue;
                    }
                }

                // Next, check for compressed episode format (103 -> S01E03, 1013 -> S10E13)
                // Only apply this if NOT preceded by a hyphen (which indicates absolute episode)
                // AND there are words before this number (compressed ep follows title)
                // AND there are no explicit SeasonEpisode tokens in the stream
                if release.seasons.is_empty() && release.episodes.is_empty() {
                    // Check if preceded by a hyphen (which suggests absolute episode)
                    let preceded_by_hyphen = if let Some(idx) = token_idx {
                        idx > 0 && matches!(tokens[idx - 1].0, Token::Hyphen)
                    } else {
                        false
                    };

                    // Check that there are word or year tokens before this number (title exists)
                    // This prevents "666 Series Title" from parsing 666 as compressed episode
                    // But allows "this.is.a.show.2015.0308" to parse 0308 as compressed episode
                    let has_title_before = if let Some(idx) = token_idx {
                        tokens[..idx]
                            .iter()
                            .any(|(t, _)| matches!(t, Token::Word(_) | Token::Year(_)))
                    } else {
                        false
                    };

                    // Allow if: not preceded by hyphen AND has title AND no explicit episode
                    if !preceded_by_hyphen && has_title_before && !has_explicit_episode {
                        if let Some((season, episode)) = parse_compressed_episode(text) {
                            // Add season
                            release.seasons.push(ParsedField::new(
                                season,
                                Confidence::HIGH,
                                (span.start, span.end),
                                *text,
                            ));
                            // Add episode
                            release.episodes.push(ParsedField::new(
                                episode,
                                Confidence::HIGH,
                                (span.start, span.end),
                                *text,
                            ));
                            release.media_type = ParsedField::new(
                                MediaType::Tv,
                                Confidence::HIGH,
                                (span.start, span.end),
                                *text,
                            );
                            continue;
                        }
                    }
                }

                // Check for absolute episode numbers (common in anime)
                // Only consider numbers 1-9999 as potential episodes
                // This runs after checking for season/episode patterns
                if release.seasons.is_empty()
                    && release.episodes.is_empty()
                    && release.absolute_episode.is_none()
                {
                    if let Ok(num) = text.parse::<u16>() {
                        if (1..=9999).contains(&num) {
                            // Check if this is likely an absolute episode
                            // by looking for nearby context
                            if is_likely_absolute_episode(tokens, span) {
                                release.absolute_episode = Some(ParsedField::new(
                                    num,
                                    Confidence::HIGH, // Increased confidence since we have context
                                    (span.start, span.end),
                                    *text,
                                ));
                                // Also add to episodes list for compatibility
                                release.episodes.push(ParsedField::new(
                                    num,
                                    Confidence::HIGH,
                                    (span.start, span.end),
                                    *text,
                                ));
                                release.media_type = ParsedField::new(
                                    MediaType::Anime,
                                    Confidence::HIGH,
                                    (span.start, span.end),
                                    *text,
                                );
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Parse season number from season-only token (e.g., "S01" -> 1).
fn parse_season_only(text: &str) -> Option<u16> {
    let upper = text.to_uppercase();
    if upper.starts_with('S') {
        let season_str = upper.trim_start_matches('S');
        season_str.parse::<u16>().ok()
    } else {
        None
    }
}

/// Parse part episode format (e.g., "Part01" -> 1, "Part1" -> 1, "Part02" -> 2).
fn parse_part_episode(text: &str) -> Option<u16> {
    let upper = text.to_uppercase();
    if upper.starts_with("PART") {
        let digits: String = upper.chars().filter(|c| c.is_ascii_digit()).collect();
        digits.parse::<u16>().ok()
    } else {
        None
    }
}

/// Parse dot-separated episode format (e.g., "e05" -> 5, "E12" -> 12).
/// This handles the episode portion of "s03.e05" style patterns.
fn parse_dot_episode(text: &str) -> Option<u16> {
    let upper = text.to_uppercase();
    // Check if it starts with 'E' followed by digits
    if upper.starts_with('E') {
        let rest = upper.trim_start_matches('E');
        // The rest should be all digits (possibly with trailing dot or chars)
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            return digits.parse::<u16>().ok();
        }
    }
    None
}

/// Parse abbreviated "Ep##" episode format (e.g., "Ep06" -> 6, "Ep1" -> 1).
fn parse_ep_number(text: &str) -> Option<u16> {
    let upper = text.to_uppercase();
    if upper.starts_with("EP") {
        let digits: String = upper
            .chars()
            .skip(2)
            .filter(|c| c.is_ascii_digit())
            .collect();
        digits.parse::<u16>().ok()
    } else {
        None
    }
}

/// Extract embedded S##E## pattern from a word (e.g., "zoos01e11" -> Some(("zoo", 1, [11]))).
/// Returns the prefix before the pattern, the season, and the episode(s).
pub fn extract_embedded_episode(text: &str) -> Option<(&str, u16, Vec<u16>)> {
    let upper = text.to_uppercase();

    // Find 's' followed by digits followed by 'e' followed by digits
    // Pattern: [prefix]s##e##[suffix]
    let mut i = 0;
    let chars: Vec<char> = upper.chars().collect();

    while i < chars.len() {
        if chars[i] == 'S' && i + 3 < chars.len() {
            // Check if followed by digits
            let mut season_end = i + 1;
            while season_end < chars.len() && chars[season_end].is_ascii_digit() {
                season_end += 1;
            }

            if season_end > i + 1 && season_end < chars.len() && chars[season_end] == 'E' {
                // Found "S##E", now find episode digits
                let mut ep_end = season_end + 1;
                while ep_end < chars.len() && chars[ep_end].is_ascii_digit() {
                    ep_end += 1;
                }

                if ep_end > season_end + 1 {
                    // We have a valid S##E## pattern
                    let season_str: String = chars[i + 1..season_end].iter().collect();
                    let episode_str: String = chars[season_end + 1..ep_end].iter().collect();

                    if let (Ok(season), Ok(episode)) =
                        (season_str.parse::<u16>(), episode_str.parse::<u16>())
                    {
                        if (1..=99).contains(&season) && (1..=9999).contains(&episode) {
                            // Only valid if there's a prefix (the 's' isn't at position 0)
                            // This distinguishes "zoos01e11" from "s01e11"
                            if i > 0 {
                                let prefix = &text[..i];
                                return Some((prefix, season, vec![episode]));
                            }
                        }
                    }
                }
            }
        }
        i += 1;
    }

    None
}

/// Parse daily episode format (e.g., "11-02" -> season 11, episode 2).
#[allow(dead_code)]
fn parse_daily_episode(text: &str) -> Option<(u16, u16)> {
    let parts: Vec<&str> = text.split('-').collect();
    if parts.len() == 2 {
        if let (Ok(season), Ok(episode)) = (parts[0].parse::<u16>(), parts[1].parse::<u16>()) {
            // Sanity check: season and episode should be reasonable
            if (1..=99).contains(&season) && (1..=99).contains(&episode) {
                return Some((season, episode));
            }
        }
    }
    None
}

/// Parse "X of Y" episode format (e.g., "5of9" -> episode 5).
fn parse_episode_of_total(text: &str) -> Option<u16> {
    let lower = text.to_lowercase();
    if let Some(of_pos) = lower.find("of") {
        let ep_str = &text[..of_pos];
        ep_str.parse::<u16>().ok()
    } else {
        None
    }
}

/// Parse compressed episode format (e.g., "103" -> S01E03, "1013" -> S10E13, "0308" -> S03E08).
///
/// Rules:
/// - 3 digits: First digit is season, last two are episode (103 -> S01E03)
/// - 4 digits: First two are season, last two are episode (1013 -> S10E13, 0308 -> S03E08)
/// - Only parse if episode is >= 1 and <= 99
fn parse_compressed_episode(text: &str) -> Option<(u16, u16)> {
    // Only process 3 or 4 digit numbers
    if text.len() != 3 && text.len() != 4 {
        return None;
    }

    // Must be all digits
    if !text.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    if text.len() == 3 {
        // 103 -> S01E03
        let num = text.parse::<u16>().ok()?;
        let season = num / 100;
        let episode = num % 100;

        // Season should be 1-9, episode should be 1-99
        if (1..=9).contains(&season) && (1..=99).contains(&episode) {
            return Some((season, episode));
        }
    } else if text.len() == 4 {
        // Parse first two chars as season, last two as episode
        // This handles both "1013" (S10E13) and "0308" (S03E08)
        let season_str = &text[0..2];
        let episode_str = &text[2..4];

        let season = season_str.parse::<u16>().ok()?;
        let episode = episode_str.parse::<u16>().ok()?;

        // Season should be 1-99, episode should be 1-99
        if (1..=99).contains(&season) && (1..=99).contains(&episode) {
            return Some((season, episode));
        }
    }

    None
}

/// Parse season and episode numbers from a season/episode token.
///
/// Supports formats:
/// - S01E05 -> (1, 5)
/// - S1E1 -> (1, 1)
/// - S01E01E02 -> (1, [1, 2])
/// - S01E01-E05 -> (1, [1, 2, 3, 4, 5])
/// - S01E12v2 -> (1, 12) [version ignored]
/// - 1x05 -> (1, 5)
fn parse_season_episode(text: &str) -> Option<(Vec<u16>, Vec<u16>)> {
    let upper = text.to_uppercase();

    // Strip version suffix if present (v2, v3, etc.)
    let clean_text = if let Some(v_pos) = upper.find('V') {
        // Check if it's followed by a number (version marker)
        let after_v = &upper[v_pos + 1..];
        if after_v.chars().all(|c| c.is_ascii_digit()) {
            &upper[..v_pos]
        } else {
            &upper
        }
    } else {
        &upper
    };

    // Try standard SxxExx format
    if clean_text.starts_with('S') && clean_text.contains('E') {
        let parts: Vec<&str> = clean_text.split('E').collect();
        if parts.len() >= 2 {
            // Extract season from first part (remove 'S')
            let season_str = parts[0].trim_start_matches('S');
            if let Ok(season) = season_str.parse::<u16>() {
                let seasons = vec![season];
                let mut episodes = Vec::new();

                // Extract all episode numbers, handling ranges like E01-E05
                // Also handle EP## format (trim leading P)
                for (i, ep_str) in parts[1..].iter().enumerate() {
                    if ep_str.is_empty() {
                        continue;
                    }

                    // Handle EP## format - strip leading P
                    let ep_str = ep_str.trim_start_matches('P');
                    if ep_str.is_empty() {
                        continue;
                    }

                    // Check for range (e.g., "01-" in "S01E01-E05")
                    if ep_str.ends_with('-') {
                        // This is the start of a range
                        if let Ok(start_ep) = ep_str.trim_end_matches('-').parse::<u16>() {
                            // Look for the end episode in the next part
                            if i + 1 < parts.len() - 1 {
                                if let Ok(end_ep) = parts[i + 2].parse::<u16>() {
                                    // Expand the range
                                    for ep in start_ep..=end_ep {
                                        if !episodes.contains(&ep) {
                                            episodes.push(ep);
                                        }
                                    }
                                    continue;
                                }
                            }
                            // If range parsing failed, just add the start
                            episodes.push(start_ep);
                        }
                    } else if let Ok(ep) = ep_str.parse::<u16>() {
                        // Regular episode number
                        if !episodes.contains(&ep) {
                            episodes.push(ep);
                        }
                    }
                }

                if !episodes.is_empty() {
                    return Some((seasons, episodes));
                }
            }
        }
    }

    // Try 1x05 format
    if let Some(x_pos) = text.find('x') {
        let season_str = &text[..x_pos];
        let episode_str = &text[x_pos + 1..];
        if let (Ok(season), Ok(episode)) = (season_str.parse::<u16>(), episode_str.parse::<u16>()) {
            // Sanity check: don't parse resolution-like patterns as season x episode
            // 1920x1080, 1920x910, 3840x2160, 1280x720 etc. are resolutions, not episodes
            // Common resolution widths to exclude
            let is_resolution_width = matches!(
                season,
                1920 | 3840 | 1280 | 2560 | 2880 | 1440 | 640 | 720 | 854
            );
            if !is_resolution_width {
                return Some((vec![season], vec![episode]));
            }
        }
    }

    None
}

/// Check if a number token is likely an absolute episode number.
///
/// Looks for context clues like:
/// - Being inside brackets (common in anime releases)
/// - Following a hyphen (e.g., "Title - 01")
/// - Being near release group brackets
fn is_likely_absolute_episode(
    tokens: &[(Token, std::ops::Range<usize>)],
    span: &std::ops::Range<usize>,
) -> bool {
    // Find the index of this token
    let token_idx = tokens
        .iter()
        .position(|(_, s)| s.start == span.start && s.end == span.end);

    if let Some(idx) = token_idx {
        // Check if preceded by a hyphen (e.g., "Anime Title - 01")
        // But NOT if it's a standalone number-hyphen-number at the START (e.g., "24-7 Series")
        if idx > 0 {
            if let Token::Hyphen = tokens[idx - 1].0 {
                // Check if the token before the hyphen is also a number
                if idx >= 2 {
                    if let Token::Number(_) = tokens[idx - 2].0 {
                        // Check if there's text/words before this number
                        // If so, it's likely "Title 2 - 05" (episode)
                        // If not, it's likely "24-7 ..." (part of title)
                        let has_text_before = tokens
                            .iter()
                            .take(idx - 2)
                            .any(|(t, _)| matches!(t, Token::Word(_)));
                        if !has_text_before {
                            return false; // "24-7" at start, not an episode
                        }
                    }
                }
                return true;
            }
        }

        // Check if surrounded by brackets
        let has_bracket_before = idx > 0 && matches!(tokens[idx - 1].0, Token::BracketOpen);
        let has_bracket_after =
            idx + 1 < tokens.len() && matches!(tokens[idx + 1].0, Token::BracketClose);

        if has_bracket_before || has_bracket_after {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_season_episode_standard() {
        let result = parse_season_episode("S01E05");
        assert_eq!(result, Some((vec![1], vec![5])));

        let result = parse_season_episode("S1E1");
        assert_eq!(result, Some((vec![1], vec![1])));
    }

    #[test]
    fn test_parse_season_episode_multi() {
        let result = parse_season_episode("S01E01E02");
        assert_eq!(result, Some((vec![1], vec![1, 2])));
    }

    #[test]
    fn test_parse_season_episode_x_format() {
        let result = parse_season_episode("1x05");
        assert_eq!(result, Some((vec![1], vec![5])));
    }

    #[test]
    fn test_extract_tv_episode() {
        let input = "Show.S01E05.720p.WEB-DL.x264-GROUP";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        extract(&lexer, &mut release);

        assert_eq!(release.seasons.len(), 1);
        assert_eq!(*release.seasons[0], 1);
        assert_eq!(release.episodes.len(), 1);
        assert_eq!(*release.episodes[0], 5);
        assert_eq!(*release.media_type, MediaType::Tv);
    }

    #[test]
    fn test_extract_compressed_episode() {
        let input = "Series.and.a.Title.103.720p.HDTV.X264-DIMENSION";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        extract(&lexer, &mut release);

        // Compressed episode 103 should be parsed as S01E03
        assert_eq!(release.seasons.len(), 1, "Should have 1 season");
        assert_eq!(*release.seasons[0], 1, "Season should be 1");
        assert_eq!(release.episodes.len(), 1, "Should have 1 episode");
        assert_eq!(*release.episodes[0], 3, "Episode should be 3");
    }

    #[test]
    fn test_extract_anime_absolute() {
        let input = "[SubGroup] Anime Title - 01 [1080p].mkv";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        extract(&lexer, &mut release);

        // The absolute episode detection should trigger
        assert!(release.absolute_episode.is_some());
    }

    #[test]
    fn test_extract_spanish_cap() {
        let input = "Cap.1901";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        extract(&lexer, &mut release);

        assert_eq!(release.seasons.len(), 1, "Should have 1 season");
        assert_eq!(*release.seasons[0], 19, "Season should be 19");
        assert_eq!(release.episodes.len(), 1, "Should have 1 episode");
        assert_eq!(*release.episodes[0], 1, "Episode should be 1");
    }

    #[test]
    fn test_extract_codec_preceded_compressed() {
        let input = "tvs-amgo-dd51-dl-7p-azhd-x264-103";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        extract(&lexer, &mut release);

        assert_eq!(release.seasons.len(), 1, "Should have 1 season");
        assert_eq!(*release.seasons[0], 1, "Season should be 1");
        assert_eq!(release.episodes.len(), 1, "Should have 1 episode");
        assert_eq!(*release.episodes[0], 3, "Episode should be 3");
    }
}
