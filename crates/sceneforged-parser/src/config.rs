//! Parser configuration.

use crate::model::MediaType;

/// How to handle ambiguous parses.
///
/// Controls behavior when the parser encounters values that could be
/// interpreted multiple ways.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AmbiguityMode {
    /// Return the best guess for each field, flagging uncertainty in confidence.
    /// This is the default behavior - always returns a value, but marks it as uncertain.
    #[default]
    BestGuess,
    /// Only return fields with high confidence (Confident or Certain).
    /// Fields with lower confidence will be left as None.
    StrictMode,
    /// Track all alternative interpretations for later resolution.
    /// Useful for integration with external media libraries.
    ReportAll,
}

/// How to handle years immediately before season/episode markers.
///
/// Controls whether "Series.2010.S01E01" has title "Series 2010" or "Series".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum YearInTitleMode {
    /// Include year in title when immediately before season/episode.
    /// "Series.2010.S01E01" → title "Series 2010"
    /// This matches Sonarr's behavior for shows like "Series Title 2010".
    #[default]
    IncludeInTitle,
    /// Treat year as metadata, not part of title.
    /// "Series.2010.S01E01" → title "Series", year 2010
    /// Better for shows like "Doctor Who (2005)" where year disambiguates.
    TreatAsMetadata,
}

/// Configuration for the parser.
///
/// Use the builder pattern to create a configuration:
///
/// ```
/// use sceneforged_parser::config::ParserConfig;
/// use sceneforged_parser::MediaType;
///
/// let config = ParserConfig::builder()
///     .media_type_hint(MediaType::Movie)
///     .build();
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParserConfig {
    /// Hint for the expected media type.
    /// If set, the parser will prefer this type when ambiguous.
    pub media_type_hint: Option<MediaType>,

    /// Expected title, if known.
    /// Helps with title boundary detection.
    pub expected_title: Option<String>,

    /// Whether to parse file extensions.
    /// Default: true
    pub parse_extensions: bool,

    /// Whether to detect anime-specific metadata.
    /// Default: true
    pub detect_anime: bool,

    /// How to handle ambiguous field values.
    /// Default: BestGuess
    pub ambiguity_mode: AmbiguityMode,

    /// How to handle years immediately before season/episode markers.
    /// Default: IncludeInTitle (matches Sonarr behavior)
    pub year_in_title: YearInTitleMode,
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self {
            media_type_hint: None,
            expected_title: None,
            parse_extensions: true,
            detect_anime: true,
            ambiguity_mode: AmbiguityMode::default(),
            year_in_title: YearInTitleMode::default(),
        }
    }
}

impl ParserConfig {
    /// Create a new default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a configuration builder.
    pub fn builder() -> ParserConfigBuilder {
        ParserConfigBuilder::default()
    }
}

/// Builder for `ParserConfig`.
#[derive(Debug, Clone, Default)]
pub struct ParserConfigBuilder {
    media_type_hint: Option<MediaType>,
    expected_title: Option<String>,
    parse_extensions: Option<bool>,
    detect_anime: Option<bool>,
    ambiguity_mode: Option<AmbiguityMode>,
    year_in_title: Option<YearInTitleMode>,
}

impl ParserConfigBuilder {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the media type hint.
    ///
    /// When the parser encounters ambiguous cases, it will prefer
    /// this media type over others.
    pub fn media_type_hint(mut self, hint: MediaType) -> Self {
        self.media_type_hint = Some(hint);
        self
    }

    /// Set the expected title.
    ///
    /// If you know the title ahead of time (e.g., from a database),
    /// setting this helps the parser identify title boundaries more accurately.
    pub fn expected_title(mut self, title: impl Into<String>) -> Self {
        self.expected_title = Some(title.into());
        self
    }

    /// Set whether to parse file extensions.
    ///
    /// Default: true
    pub fn parse_extensions(mut self, enabled: bool) -> Self {
        self.parse_extensions = Some(enabled);
        self
    }

    /// Set whether to detect anime-specific metadata.
    ///
    /// When enabled, the parser looks for CRC32 checksums, fansub groups,
    /// and anime versioning (v2, v3, etc.).
    ///
    /// Default: true
    pub fn detect_anime(mut self, enabled: bool) -> Self {
        self.detect_anime = Some(enabled);
        self
    }

    /// Set how to handle ambiguous field values.
    ///
    /// - `BestGuess` (default): Always return a value, flag uncertainty in confidence
    /// - `StrictMode`: Only return high-confidence values, leave uncertain fields as None
    /// - `ReportAll`: Track all alternatives for external resolution
    pub fn ambiguity_mode(mut self, mode: AmbiguityMode) -> Self {
        self.ambiguity_mode = Some(mode);
        self
    }

    /// Set how to handle years immediately before season/episode markers.
    ///
    /// - `IncludeInTitle` (default): "Series.2010.S01E01" → title "Series 2010"
    /// - `TreatAsMetadata`: "Series.2010.S01E01" → title "Series", year 2010
    ///
    /// Use `IncludeInTitle` for Sonarr-style naming where years disambiguate series.
    /// Use `TreatAsMetadata` for shows like "Doctor Who (2005)" where the year is metadata.
    pub fn year_in_title(mut self, mode: YearInTitleMode) -> Self {
        self.year_in_title = Some(mode);
        self
    }

    /// Build the configuration.
    pub fn build(self) -> ParserConfig {
        ParserConfig {
            media_type_hint: self.media_type_hint,
            expected_title: self.expected_title,
            parse_extensions: self.parse_extensions.unwrap_or(true),
            detect_anime: self.detect_anime.unwrap_or(true),
            ambiguity_mode: self.ambiguity_mode.unwrap_or_default(),
            year_in_title: self.year_in_title.unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ParserConfig::default();
        assert!(config.media_type_hint.is_none());
        assert!(config.expected_title.is_none());
        assert!(config.parse_extensions);
        assert!(config.detect_anime);
    }

    #[test]
    fn test_builder_pattern() {
        let config = ParserConfig::builder()
            .media_type_hint(MediaType::Movie)
            .expected_title("The Matrix")
            .parse_extensions(false)
            .detect_anime(false)
            .build();

        assert_eq!(config.media_type_hint, Some(MediaType::Movie));
        assert_eq!(config.expected_title, Some("The Matrix".to_string()));
        assert!(!config.parse_extensions);
        assert!(!config.detect_anime);
    }

    #[test]
    fn test_builder_partial() {
        let config = ParserConfig::builder()
            .media_type_hint(MediaType::Tv)
            .build();

        assert_eq!(config.media_type_hint, Some(MediaType::Tv));
        assert!(config.expected_title.is_none());
        assert!(config.parse_extensions); // default
        assert!(config.detect_anime); // default
    }
}
