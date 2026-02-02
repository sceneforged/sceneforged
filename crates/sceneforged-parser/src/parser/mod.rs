//! Winnow-based parser for release names.
//!
//! This module provides parsers that operate on the token stream from the lexer,
//! using winnow combinators to extract structured metadata.

mod codec;
mod episode;
mod metadata;
mod quality;
mod title;

use crate::config::ParserConfig;
use crate::lexer::Lexer;
use crate::model::{Confidence, MediaType, ParsedField, ParsedRelease};

/// Parse a release name into structured metadata using default config.
///
/// This is a convenience function that uses default configuration.
/// For custom configuration, use [`parse_with_config`].
pub fn parse(input: &str) -> ParsedRelease {
    parse_with_config(input, &ParserConfig::default())
}

/// Parse a release name into structured metadata with custom configuration.
///
/// This is the main entry point for the winnow-based parser. It tokenizes
/// the input using the Logos lexer, then applies specialized parsers to
/// extract each type of metadata.
///
/// Parsers are applied in order of specificity to ensure that more specific
/// patterns are matched before more general ones.
pub fn parse_with_config(input: &str, config: &ParserConfig) -> ParsedRelease {
    let lexer = Lexer::new(input);
    let mut release = ParsedRelease::new(input);

    // Apply parsers in order of specificity
    // More specific parsers (episode, quality) go first to avoid
    // having their tokens consumed by the title parser
    episode::extract(&lexer, &mut release);
    quality::extract(&lexer, &mut release);
    codec::extract(&lexer, &mut release);
    metadata::extract_with_config(&lexer, &mut release, config);
    title::extract_with_config(&lexer, &mut release, config);

    // Post-processing: Determine media type based on collected info
    // This runs after all extraction to use complete information
    if release.seasons.is_empty()
        && release.episodes.is_empty()
        && release.absolute_episode.is_none()
        && release.year.is_some()
        && *release.media_type == MediaType::Unknown
    {
        let input_len = lexer.input().len();
        release.media_type =
            ParsedField::new(MediaType::Movie, Confidence::HIGH, (0, input_len), "");
    }

    release
}
