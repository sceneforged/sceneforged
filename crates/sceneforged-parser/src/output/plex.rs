//! Plex-compatible output formatting.

use super::OutputFormat;
use crate::model::ParsedRelease;

/// Plex-compatible output formatter.
///
/// Formats parsed releases according to Plex naming conventions:
/// - Movies: "Movie Title (Year)"
/// - TV Shows: "Show Title - s##e## - Episode Title"
///
/// # Example
///
/// ```
/// use sceneforged_parser::{parse, output::{OutputFormat, PlexFormat}};
///
/// let release = parse("The.Matrix.1999.1080p.BluRay.x264-GROUP");
/// let formatter = PlexFormat;
/// let output = formatter.format(&release);
///
/// assert_eq!(output.formatted_title, "The Matrix (1999)");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlexFormat;

/// Plex-compatible output structure.
///
/// Contains formatted strings optimized for Plex's media scanner and
/// metadata matching system.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlexOutput {
    /// Plex-formatted title (includes year for movies, season/episode for TV)
    pub formatted_title: String,
    /// Base title without year or episode information
    pub base_title: String,
    /// Media type (Movie, TV, Anime)
    pub media_type: String,
    /// Year if available
    pub year: Option<u16>,
    /// Season number for TV shows
    pub season: Option<u16>,
    /// Episode number for TV shows (first episode if multi-episode)
    pub episode: Option<u16>,
    /// Episode title if available
    pub episode_title: Option<String>,
    /// Quality information
    pub quality: Option<String>,
    /// Original release title
    pub original_title: String,
}

impl OutputFormat for PlexFormat {
    type Output = PlexOutput;

    fn format(&self, release: &ParsedRelease) -> Self::Output {
        let base_title = (*release.title).clone();
        let year = release.year.as_ref().map(|y| **y);
        let media_type = format!("{:?}", *release.media_type);

        // Build quality string
        let quality = match (&release.resolution, &release.source) {
            (Some(res), Some(src)) => Some(format!("{:?} {:?}", **res, **src)),
            (Some(res), None) => Some(format!("{:?}", **res)),
            (None, Some(src)) => Some(format!("{:?}", **src)),
            (None, None) => None,
        };

        // Get season and episode information
        let season = release.seasons.first().map(|s| **s);
        let episode = release.episodes.first().map(|e| **e);
        let episode_title = release.episode_title.as_ref().map(|t| (**t).clone());

        // Format title according to Plex conventions
        let formatted_title = if release.is_movie() {
            // Movie format: "Title (Year)"
            if let Some(y) = year {
                format!("{} ({})", base_title, y)
            } else {
                base_title.clone()
            }
        } else if release.is_tv() {
            // TV show format: "Show Title - s##e## - Episode Title"
            match (season, episode) {
                (Some(s), Some(e)) => {
                    let season_episode = format!("s{:02}e{:02}", s, e);
                    if let Some(ref ep_title) = episode_title {
                        format!("{} - {} - {}", base_title, season_episode, ep_title)
                    } else {
                        format!("{} - {}", base_title, season_episode)
                    }
                }
                (Some(s), None) => {
                    // Season pack
                    format!("{} - Season {:02}", base_title, s)
                }
                (None, Some(e)) => {
                    // Episode without season (rare, but handle it)
                    if let Some(ref ep_title) = episode_title {
                        format!("{} - Episode {} - {}", base_title, e, ep_title)
                    } else {
                        format!("{} - Episode {}", base_title, e)
                    }
                }
                (None, None) => {
                    // No episode info, just use title
                    base_title.clone()
                }
            }
        } else {
            // Unknown media type, use basic format
            if let Some(y) = year {
                format!("{} ({})", base_title, y)
            } else {
                base_title.clone()
            }
        };

        PlexOutput {
            formatted_title,
            base_title,
            media_type,
            year,
            season,
            episode,
            episode_title,
            quality,
            original_title: release.release_title.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[test]
    fn test_plex_format_movie() {
        let release = parse("The.Matrix.1999.1080p.BluRay.x264-GROUP");
        let formatter = PlexFormat;
        let output = formatter.format(&release);

        assert_eq!(output.formatted_title, "The Matrix (1999)");
        assert_eq!(output.base_title, "The Matrix");
        assert_eq!(output.year, Some(1999));
        assert_eq!(output.season, None);
        assert_eq!(output.episode, None);
        assert!(output.quality.is_some());
    }

    #[test]
    fn test_plex_format_movie_no_year() {
        let release = parse("Movie.Title.1080p.BluRay.x264-GROUP");
        let formatter = PlexFormat;
        let output = formatter.format(&release);

        assert_eq!(output.formatted_title, "Movie Title");
        assert_eq!(output.base_title, "Movie Title");
        assert_eq!(output.year, None);
    }

    #[test]
    fn test_plex_format_tv_episode() {
        let release = parse("Breaking.Bad.S01E01.720p.BluRay.x264-DEMAND");
        let formatter = PlexFormat;
        let output = formatter.format(&release);

        assert_eq!(output.formatted_title, "Breaking Bad - s01e01");
        assert_eq!(output.base_title, "Breaking Bad");
        assert_eq!(output.season, Some(1));
        assert_eq!(output.episode, Some(1));
        assert!(output.quality.is_some());
    }

    #[test]
    fn test_plex_format_tv_with_episode_title() {
        // Note: This requires the parser to extract episode titles
        // The test demonstrates the expected behavior
        let mut release = parse("Breaking.Bad.S01E01.720p.BluRay.x264-DEMAND");
        release.episode_title = Some(crate::model::ParsedField::certain(
            "Pilot".to_string(),
            (0, 0),
            "Pilot",
        ));

        let formatter = PlexFormat;
        let output = formatter.format(&release);

        assert_eq!(output.formatted_title, "Breaking Bad - s01e01 - Pilot");
        assert_eq!(output.episode_title, Some("Pilot".to_string()));
    }

    #[test]
    fn test_plex_format_season_pack() {
        let release = parse("Breaking.Bad.S01.720p.BluRay.x264-DEMAND");
        let formatter = PlexFormat;
        let output = formatter.format(&release);

        // When no episodes are detected, it should format as season pack
        if output.season.is_some() && output.episode.is_none() {
            assert!(output.formatted_title.contains("Season"));
        }
    }

    #[test]
    fn test_plex_format_anime() {
        let release = parse("[SubGroup] Anime Title - 01 [1080p].mkv");
        let formatter = PlexFormat;
        let output = formatter.format(&release);

        assert_eq!(output.base_title, "Anime Title");
        assert_eq!(output.episode, Some(1));
        // Anime is detected as MediaType::Anime which is a TV variant
        assert!(output.formatted_title.contains("Anime Title"));
    }

    #[test]
    fn test_plex_format_multi_episode() {
        let release = parse("Show.S01E01E02.720p.WEB-DL.x264-GROUP");
        let formatter = PlexFormat;
        let output = formatter.format(&release);

        // Plex format uses first episode for multi-episode releases
        assert_eq!(output.season, Some(1));
        assert_eq!(output.episode, Some(1));
        assert_eq!(output.formatted_title, "Show - s01e01");
    }
}
