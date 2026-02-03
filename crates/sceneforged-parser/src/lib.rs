//! # sceneforged-parser
//!
//! A fast, comprehensive parser for media release names.
//!
//! This crate provides functionality to parse release names commonly found
//! in media files and extract structured metadata including title, year,
//! quality, codecs, languages, and more.
//!
//! ## Quick Start
//!
//! ```
//! use sceneforged_parser::parse;
//!
//! let result = parse("The.Matrix.1999.1080p.BluRay.x264-GROUP");
//!
//! assert_eq!(*result.title, "The Matrix");
//! assert!(result.year.is_some());
//! assert_eq!(**result.year.as_ref().unwrap(), 1999);
//! ```
//!
//! ## Configurable Parsing
//!
//! ```
//! use sceneforged_parser::{Parser, MediaType};
//! use sceneforged_parser::config::ParserConfig;
//!
//! let config = ParserConfig::builder()
//!     .media_type_hint(MediaType::Movie)
//!     .build();
//!
//! let parser = Parser::new(config);
//! let result = parser.parse("Ambiguous.Title.2020.720p");
//! ```

pub mod config;
pub mod model;
pub mod output;

pub mod lexer;
mod parser;

#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
pub mod wasm;

// Re-export main types for convenience
pub use model::{
    AudioChannels, AudioCodec, Confidence, Edition, FrameRate, HdrFormat, Language, MediaType,
    OptionalField, ParseError, ParsedField, ParsedRelease, QualityModifier, Resolution, Revision,
    Source, StreamingService, VideoEncoder, VideoStandard,
};

use config::ParserConfig;
pub use config::{AmbiguityMode, YearInTitleMode};

/// Parse a release name into structured metadata using default settings.
///
/// This is the simplest way to parse a release name. For more control,
/// use [`Parser`] with a custom [`ParserConfig`].
///
/// # Examples
///
/// ```
/// use sceneforged_parser::parse;
///
/// let result = parse("The.Matrix.1999.1080p.BluRay.x264-GROUP");
/// assert_eq!(*result.title, "The Matrix");
/// assert!(result.year.is_some());
/// assert_eq!(**result.year.as_ref().unwrap(), 1999);
/// ```
pub fn parse(input: &str) -> ParsedRelease {
    Parser::default().parse(input)
}

/// A configurable release name parser.
///
/// Create a `Parser` with custom settings using [`ParserConfig`]:
///
/// ```
/// use sceneforged_parser::{Parser, MediaType};
/// use sceneforged_parser::config::ParserConfig;
///
/// let config = ParserConfig::builder()
///     .media_type_hint(MediaType::Tv)
///     .build();
///
/// let parser = Parser::new(config);
/// ```
#[derive(Debug, Clone)]
pub struct Parser {
    config: ParserConfig,
}

impl Parser {
    /// Create a new parser with the given configuration.
    pub fn new(config: ParserConfig) -> Self {
        Self { config }
    }

    /// Parse a release name into structured metadata.
    ///
    /// # Arguments
    /// * `input` - The release name string to parse
    ///
    /// # Returns
    /// A [`ParsedRelease`] containing all extracted metadata
    ///
    /// # Examples
    ///
    /// ```
    /// use sceneforged_parser::Parser;
    ///
    /// let parser = Parser::default();
    /// let result = parser.parse("Movie.2020.1080p.BluRay.x264-GROUP");
    /// assert!(result.year.is_some());
    /// assert_eq!(**result.year.as_ref().unwrap(), 2020);
    /// ```
    pub fn parse(&self, input: &str) -> ParsedRelease {
        // Use the winnow-based parser with config
        parser::parse_with_config(input, &self.config)
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new(ParserConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_movie() {
        let result = parse("The.Matrix.1999.1080p.BluRay.x264-GROUP");
        assert_eq!(*result.title, "The Matrix");
        assert!(result.year.is_some());
        assert_eq!(**result.year.as_ref().unwrap(), 1999);
        assert!(result.resolution.is_some());
        assert_eq!(**result.resolution.as_ref().unwrap(), Resolution::_1080p);
        assert!(result.source.is_some());
        assert_eq!(**result.source.as_ref().unwrap(), Source::BluRay);
        assert!(result.video_encoder.is_some());
        assert_eq!(**result.video_encoder.as_ref().unwrap(), VideoEncoder::X264);
        assert!(result.release_group.is_some());
        assert_eq!(**result.release_group.as_ref().unwrap(), "GROUP");
        assert_eq!(*result.media_type, MediaType::Movie);
    }

    #[test]
    fn test_parse_tv_episode() {
        let result = parse("Breaking.Bad.S01E01.720p.BluRay.x264-DEMAND");
        assert_eq!(*result.title, "Breaking Bad");
        assert_eq!(result.seasons.len(), 1);
        assert_eq!(*result.seasons[0], 1);
        assert_eq!(result.episodes.len(), 1);
        assert_eq!(*result.episodes[0], 1);
        assert!(result.resolution.is_some());
        assert_eq!(**result.resolution.as_ref().unwrap(), Resolution::_720p);
        assert_eq!(*result.media_type, MediaType::Tv);
    }

    #[test]
    fn test_parse_4k_movie() {
        let result = parse("Inception.2010.2160p.UHD.BluRay.x265.HDR.DTS-HD.MA.5.1-RELEASE");
        assert_eq!(*result.title, "Inception");
        assert!(result.year.is_some());
        assert_eq!(**result.year.as_ref().unwrap(), 2010);
        assert!(result.resolution.is_some());
        assert_eq!(**result.resolution.as_ref().unwrap(), Resolution::_2160p);
        assert!(result.video_encoder.is_some());
        assert_eq!(**result.video_encoder.as_ref().unwrap(), VideoEncoder::X265);
        assert!(result.audio_codec.is_some());
        assert_eq!(**result.audio_codec.as_ref().unwrap(), AudioCodec::DtsHdMa);
        assert!(result.audio_channels.is_some());
        assert_eq!(
            **result.audio_channels.as_ref().unwrap(),
            AudioChannels::_5_1
        );
    }

    #[test]
    fn test_parse_anime() {
        let result = parse("[SubGroup] Anime Title - 01 [1080p] [ABCD1234].mkv");
        // Should detect anime based on CRC32 checksum or absolute episode
        assert!(result.file_checksum.is_some() || result.absolute_episode.is_some());
    }

    #[test]
    fn test_parse_web_release() {
        let result = parse("Movie.2023.1080p.AMZN.WEB-DL.DDP5.1.H.264-GROUP");
        assert!(result.source.is_some());
        assert_eq!(**result.source.as_ref().unwrap(), Source::WebDl);
        // Note: Streaming service extraction is not yet implemented
        // assert_eq!(result.streaming_service, Some(StreamingService::Amazon));
    }

    #[test]
    fn test_parse_multi_episode() {
        let result = parse("Show.S01E01E02.720p.WEB-DL.x264-GROUP");
        assert_eq!(result.seasons.len(), 1);
        assert_eq!(*result.seasons[0], 1);
        assert!(result.episodes.iter().any(|e| **e == 1));
        assert!(result.episodes.iter().any(|e| **e == 2));
    }

    #[test]
    fn test_parser_with_config() {
        let config = ParserConfig::builder()
            .media_type_hint(MediaType::Movie)
            .build();
        let parser = Parser::new(config);
        let result = parser.parse("Ambiguous.2020.720p.WEB-DL");
        assert_eq!(*result.media_type, MediaType::Movie);
    }

    #[test]
    fn test_release_title_preserved() {
        let input = "Some.Movie.2021.1080p.WEB-DL";
        let result = parse(input);
        assert_eq!(result.release_title, input);
    }

    #[test]
    fn test_hdr_detection() {
        // Note: HDR detection is not yet fully implemented in extractors
        // This test documents expected future behavior
        let result = parse("Movie.2020.2160p.BluRay.x265.HDR10-GROUP");
        // For now just verify parsing doesn't crash
        assert!(result.resolution.is_some());
        assert_eq!(**result.resolution.as_ref().unwrap(), Resolution::_2160p);
        assert!(result.video_encoder.is_some());
        assert_eq!(**result.video_encoder.as_ref().unwrap(), VideoEncoder::X265);
    }

    #[test]
    fn test_episode_format_1x01() {
        let result = parse("Show.1x01.720p.HDTV");
        assert_eq!(result.seasons.len(), 1);
        assert_eq!(*result.seasons[0], 1);
        assert_eq!(result.episodes.len(), 1);
        assert_eq!(*result.episodes[0], 1);
    }

    #[test]
    fn test_year_before_season_as_title() {
        // Default behavior (IncludeInTitle): year is included in the title
        // This matches Sonarr's expectation (e.g., "Series Title 2010" should include year)
        let result = parse("Shogun.2024.S01E10.720p.HDTV");
        assert_eq!(*result.title, "Shogun 2024");
        // Year is NOT extracted as metadata when part of title
        assert!(result.year.is_none());
        assert_eq!(result.seasons.len(), 1);
        assert_eq!(*result.seasons[0], 1);
        assert_eq!(result.episodes.len(), 1);
        assert_eq!(*result.episodes[0], 10);
    }

    #[test]
    fn test_year_before_season_as_metadata() {
        // TreatAsMetadata mode: year is extracted as metadata, not part of title
        // Better for shows like "Doctor Who (2005)" where year disambiguates
        use crate::config::{ParserConfig, YearInTitleMode};

        let config = ParserConfig::builder()
            .year_in_title(YearInTitleMode::TreatAsMetadata)
            .build();
        let parser = Parser::new(config);
        let result = parser.parse("Shogun.2024.S01E10.720p.HDTV");

        assert_eq!(*result.title, "Shogun");
        assert!(result.year.is_some());
        assert_eq!(**result.year.as_ref().unwrap(), 2024);
        assert_eq!(result.seasons.len(), 1);
        assert_eq!(*result.seasons[0], 1);
        assert_eq!(result.episodes.len(), 1);
        assert_eq!(*result.episodes[0], 10);
    }

    #[test]
    fn test_parse_movie_simple() {
        let result = parse("Movie.Title.2024.1080p.BluRay.x264-GROUP");
        assert_eq!(*result.title, "Movie Title");
        assert!(result.year.is_some());
        assert_eq!(**result.year.as_ref().unwrap(), 2024);
        assert!(result.resolution.is_some());
        assert_eq!(**result.resolution.as_ref().unwrap(), Resolution::_1080p);
        assert!(result.source.is_some());
        assert_eq!(**result.source.as_ref().unwrap(), Source::BluRay);
        assert!(result.video_encoder.is_some());
        assert_eq!(**result.video_encoder.as_ref().unwrap(), VideoEncoder::X264);
    }

    #[test]
    fn test_parse_container() {
        let result = parse("Movie.2020.1080p.BluRay.x264-GROUP.mkv");
        assert!(result.container.is_some());
        assert_eq!(**result.container.as_ref().unwrap(), "mkv");
    }

    #[test]
    fn test_default_parser() {
        let parser = Parser::default();
        let result = parser.parse("Test.2020.1080p");
        assert!(result.year.is_some());
        assert_eq!(**result.year.as_ref().unwrap(), 2020);
    }

    // Tests for audio codec detection from fixture patterns

    #[test]
    fn test_parse_dd51_from_fixture() {
        let result = parse("Interstellar.2014.720p.WEB-DL.DD5.1.H.264-SPARKS");
        assert!(result.audio_codec.is_some());
        assert_eq!(**result.audio_codec.as_ref().unwrap(), AudioCodec::Ac3);
        assert!(result.audio_channels.is_some());
        assert_eq!(
            **result.audio_channels.as_ref().unwrap(),
            AudioChannels::_5_1
        );
    }

    #[test]
    fn test_parse_truehd_71_atmos_from_fixture() {
        let result =
            parse("The.Dark.Knight.2008.2160p.UHD.BluRay.REMUX.HDR.HEVC.TrueHD.7.1.Atmos-FGT");
        assert!(result.audio_codec.is_some());
        assert_eq!(
            **result.audio_codec.as_ref().unwrap(),
            AudioCodec::TrueHdAtmos
        );
        assert!(result.audio_channels.is_some());
        assert_eq!(
            **result.audio_channels.as_ref().unwrap(),
            AudioChannels::_7_1
        );
    }

    #[test]
    fn test_parse_ddp51_atmos_from_fixture() {
        let result = parse("Forrest.Gump.1994.2160p.WEB-DL.DDP.5.1.Atmos.DV.x265-FLUX");
        assert!(result.audio_codec.is_some());
        assert_eq!(
            **result.audio_codec.as_ref().unwrap(),
            AudioCodec::Eac3Atmos
        );
        assert!(result.audio_channels.is_some());
        assert_eq!(
            **result.audio_channels.as_ref().unwrap(),
            AudioChannels::_5_1
        );
    }

    #[test]
    fn test_parse_ddp51_from_fixture() {
        let result = parse("The.Prestige.2006.2160p.WEBRip.x265.10bit.HDR.DDP.5.1-GROUP");
        assert!(result.audio_codec.is_some());
        assert_eq!(**result.audio_codec.as_ref().unwrap(), AudioCodec::Eac3);
        assert!(result.audio_channels.is_some());
        assert_eq!(
            **result.audio_channels.as_ref().unwrap(),
            AudioChannels::_5_1
        );
    }
}

#[cfg(test)]
mod anime_tests {
    use super::*;

    #[test]
    fn test_anime_full_parsing() {
        let input = "[SubGroup] Anime Title - 01 [1080p].mkv";
        let result = parse(input);

        assert!(result.release_group.is_some());
        assert_eq!(**result.release_group.as_ref().unwrap(), "SubGroup");
        assert_eq!(*result.title, "Anime Title");
        assert_eq!(result.episodes.len(), 1);
        assert_eq!(*result.episodes[0], 1);
        assert!(result.absolute_episode.is_some());
        assert_eq!(**result.absolute_episode.as_ref().unwrap(), 1);
        assert!(result.resolution.is_some());
        assert_eq!(**result.resolution.as_ref().unwrap(), Resolution::_1080p);
        assert!(result.container.is_some());
        assert_eq!(**result.container.as_ref().unwrap(), "mkv");
        assert_eq!(*result.media_type, MediaType::Anime);
    }

    #[test]
    fn test_anime_with_checksum() {
        let input = "[SubsPlease] Jujutsu Kaisen - 24 (1080p) [ABCD1234].mkv";
        let result = parse(input);

        assert!(result.release_group.is_some());
        assert_eq!(**result.release_group.as_ref().unwrap(), "SubsPlease");
        assert_eq!(*result.title, "Jujutsu Kaisen");
        assert_eq!(result.episodes.len(), 1);
        assert_eq!(*result.episodes[0], 24);
        assert!(result.file_checksum.is_some());
        assert_eq!(**result.file_checksum.as_ref().unwrap(), "ABCD1234");
    }

    #[test]
    fn test_anime_with_season_episode() {
        let input = "[Judas] Chainsaw Man - S01E12 [1080p][HEVC x265 10bit][Dual-Audio].mkv";
        let result = parse(input);

        assert!(result.release_group.is_some());
        assert_eq!(**result.release_group.as_ref().unwrap(), "Judas");
        assert_eq!(*result.title, "Chainsaw Man");
        assert_eq!(result.seasons.len(), 1);
        assert_eq!(*result.seasons[0], 1);
        assert_eq!(result.episodes.len(), 1);
        assert_eq!(*result.episodes[0], 12);
    }
}
