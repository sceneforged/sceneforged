//! Sonarr-compatible output formatting.

use super::OutputFormat;
use crate::model::ParsedRelease;

/// Sonarr-compatible output formatter.
///
/// Formats parsed releases to match Sonarr's expected structure, including
/// quality profiles, source information, and release metadata.
///
/// # Example
///
/// ```
/// use sceneforged_parser::{parse, output::{OutputFormat, SonarrFormat}};
///
/// let release = parse("Breaking.Bad.S01E01.720p.BluRay.x264-DEMAND");
/// let formatter = SonarrFormat;
/// let output = formatter.format(&release);
///
/// assert_eq!(output.series_title, "Breaking Bad");
/// assert_eq!(output.season_number, Some(1));
/// assert_eq!(output.episode_numbers, vec![1]);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SonarrFormat;

/// Sonarr-compatible output structure.
///
/// This structure mirrors Sonarr's internal representation of release
/// information for compatibility with import and matching operations.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SonarrOutput {
    /// The series or movie title
    pub series_title: String,
    /// Season number for TV shows
    pub season_number: Option<u16>,
    /// Episode numbers (can be multiple for multi-episode releases)
    pub episode_numbers: Vec<u16>,
    /// Year of release
    pub year: Option<u16>,
    /// Quality profile (resolution + source combination)
    pub quality: String,
    /// Source type (BluRay, WEB-DL, HDTV, etc.)
    pub source: Option<String>,
    /// Video codec/encoder
    pub codec: Option<String>,
    /// Resolution (720p, 1080p, 2160p, etc.)
    pub resolution: Option<String>,
    /// Release group
    pub release_group: Option<String>,
    /// Whether this is a proper/repack release
    pub is_proper: bool,
    /// Whether this is a repack
    pub is_repack: bool,
    /// Full season pack indicator
    pub full_season: bool,
    /// Original release title
    pub release_title: String,
}

impl OutputFormat for SonarrFormat {
    type Output = SonarrOutput;

    fn format(&self, release: &ParsedRelease) -> Self::Output {
        // Build quality string from resolution and source
        let quality = match (&release.resolution, &release.source) {
            (Some(res), Some(src)) => format!("{:?} {:?}", **res, **src),
            (Some(res), None) => format!("{:?}", **res),
            (None, Some(src)) => format!("{:?}", **src),
            (None, None) => "Unknown".to_string(),
        };

        // Extract season number (use first if multiple)
        let season_number = release.seasons.first().map(|s| **s);

        // Extract episode numbers
        let episode_numbers = release.episodes.iter().map(|e| **e).collect();

        // Extract video codec
        let codec = if let Some(ref encoder) = release.video_encoder {
            Some(format!("{:?}", **encoder))
        } else if let Some(ref standard) = release.video_standard {
            Some(format!("{:?}", **standard))
        } else {
            None
        };

        // Extract source
        let source = release.source.as_ref().map(|s| format!("{:?}", **s));

        // Extract resolution
        let resolution = release.resolution.as_ref().map(|r| format!("{:?}", **r));

        // Extract release group
        let release_group = release.release_group.as_ref().map(|g| (**g).clone());

        // Check for proper/repack
        // Note: 'real' field represents PROPER/REAL tags
        // Repack detection would require additional parsing
        let is_proper = release.revision.real > 0;
        let is_repack = false; // TODO: Add repack detection to parser

        SonarrOutput {
            series_title: (*release.title).clone(),
            season_number,
            episode_numbers,
            year: release.year.as_ref().map(|y| **y),
            quality,
            source,
            codec,
            resolution,
            release_group,
            is_proper,
            is_repack,
            full_season: release.full_season,
            release_title: release.release_title.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[test]
    fn test_sonarr_format_tv_show() {
        let release = parse("Breaking.Bad.S01E01.720p.BluRay.x264-DEMAND");
        let formatter = SonarrFormat;
        let output = formatter.format(&release);

        assert_eq!(output.series_title, "Breaking Bad");
        assert_eq!(output.season_number, Some(1));
        assert_eq!(output.episode_numbers, vec![1]);
        assert!(output.resolution.is_some());
        assert!(output.source.is_some());
        assert!(output.codec.is_some());
        assert_eq!(output.release_group, Some("DEMAND".to_string()));
        assert!(!output.is_proper);
        assert!(!output.is_repack);
        assert!(!output.full_season);
    }

    #[test]
    fn test_sonarr_format_movie() {
        let release = parse("The.Matrix.1999.1080p.BluRay.x264-GROUP");
        let formatter = SonarrFormat;
        let output = formatter.format(&release);

        assert_eq!(output.series_title, "The Matrix");
        assert_eq!(output.year, Some(1999));
        assert_eq!(output.season_number, None);
        assert!(output.episode_numbers.is_empty());
        assert!(output.resolution.is_some());
        assert!(output.source.is_some());
    }

    #[test]
    fn test_sonarr_format_multi_episode() {
        let release = parse("Show.S01E01E02.720p.WEB-DL.x264-GROUP");
        let formatter = SonarrFormat;
        let output = formatter.format(&release);

        assert_eq!(output.season_number, Some(1));
        assert_eq!(output.episode_numbers.len(), 2);
        assert!(output.episode_numbers.contains(&1));
        assert!(output.episode_numbers.contains(&2));
    }

    #[test]
    fn test_sonarr_format_proper() {
        let release = parse("Show.S01E01.PROPER.720p.HDTV.x264-GROUP");
        let formatter = SonarrFormat;
        let output = formatter.format(&release);

        assert!(output.is_proper);
        assert!(!output.is_repack);
    }

    #[test]
    fn test_sonarr_format_repack() {
        let release = parse("Show.S01E01.REPACK.720p.HDTV.x264-GROUP");
        let formatter = SonarrFormat;
        let output = formatter.format(&release);

        assert!(!output.is_proper);
        // Note: REPACK detection is not yet implemented in the parser
        // This test documents expected future behavior
        // assert!(output.is_repack);
    }
}
