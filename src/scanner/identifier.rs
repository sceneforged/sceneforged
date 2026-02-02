//! Media identification using sceneforged-parser.
//!
//! This module provides functionality for identifying media files from their
//! filenames by parsing release name conventions.

use sceneforged_parser::{parse, MediaType as ParserMediaType, ParsedRelease};
use std::path::Path;

/// Media identifier that parses release names.
#[derive(Debug, Default)]
pub struct MediaIdentifier;

/// Identification result from parsing a filename.
#[derive(Debug, Clone)]
pub struct IdentificationResult {
    /// The full parsed release data.
    pub parsed: ParsedRelease,
    /// Cleaned title ready for metadata lookup.
    pub title: String,
    /// Year if detected.
    pub year: Option<u16>,
    /// Season number (first if multiple).
    pub season: Option<u16>,
    /// Episode number (first if multiple).
    pub episode: Option<u16>,
    /// Resolution as string for Item.resolution field.
    pub resolution_str: Option<String>,
    /// Source as string.
    pub source_str: Option<String>,
    /// Release group name.
    pub release_group: Option<String>,
    /// Scene release name (original filename).
    pub scene_release_name: String,
    /// Detected media type from parser.
    pub parser_media_type: ParserMediaType,
}

impl MediaIdentifier {
    /// Create a new media identifier.
    pub fn new() -> Self {
        Self
    }

    /// Parse a filename to extract media information.
    pub fn identify_from_filename(&self, path: &Path) -> IdentificationResult {
        let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

        let parsed = parse(filename);

        IdentificationResult {
            title: (*parsed.title).clone(),
            year: parsed.year.as_ref().map(|f| **f),
            season: parsed.seasons.first().map(|f| **f),
            episode: parsed.episodes.first().map(|f| **f),
            // Use Display implementations from the parser enums
            resolution_str: parsed.resolution.as_ref().map(|r| (**r).to_string()),
            source_str: parsed.source.as_ref().map(|s| (**s).to_string()),
            release_group: parsed.release_group.as_ref().map(|g| (**g).clone()),
            scene_release_name: parsed.release_title.clone(),
            parser_media_type: (*parsed.media_type).clone(),
            parsed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_identify_movie() {
        let identifier = MediaIdentifier::new();
        let path = PathBuf::from("The.Matrix.1999.2160p.UHD.BluRay.REMUX-GROUP.mkv");

        let result = identifier.identify_from_filename(&path);
        assert_eq!(result.title, "The Matrix");
        assert_eq!(result.year, Some(1999));
        assert_eq!(result.resolution_str, Some("2160p".to_string()));
        assert_eq!(result.source_str, Some("BluRay".to_string()));
        assert_eq!(result.release_group, Some("GROUP".to_string()));
    }

    #[test]
    fn test_identify_tv_show() {
        let identifier = MediaIdentifier::new();
        let path = PathBuf::from("Breaking.Bad.S01E01.1080p.BluRay-DEMAND.mkv");

        let result = identifier.identify_from_filename(&path);
        assert_eq!(result.title, "Breaking Bad");
        assert_eq!(result.season, Some(1));
        assert_eq!(result.episode, Some(1));
        assert_eq!(result.resolution_str, Some("1080p".to_string()));
    }

    #[test]
    fn test_identify_anime() {
        let identifier = MediaIdentifier::new();
        let path = PathBuf::from("[SubGroup] Anime Title - 01 [1080p].mkv");

        let result = identifier.identify_from_filename(&path);
        assert_eq!(result.title, "Anime Title");
        assert_eq!(result.episode, Some(1));
        assert_eq!(result.release_group, Some("SubGroup".to_string()));
    }

    #[test]
    fn test_scene_release_name_preserved() {
        let identifier = MediaIdentifier::new();
        let path = PathBuf::from("Movie.2020.1080p.BluRay.x264-GROUP.mkv");

        let result = identifier.identify_from_filename(&path);
        assert_eq!(
            result.scene_release_name,
            "Movie.2020.1080p.BluRay.x264-GROUP"
        );
    }
}
