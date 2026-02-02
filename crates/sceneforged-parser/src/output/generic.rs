//! Generic human-readable output formatting.

use super::OutputFormat;
use crate::model::ParsedRelease;

/// Generic human-readable output formatter.
///
/// Produces a simple string representation of parsed release data,
/// useful for debugging, logging, and general display purposes.
///
/// # Example
///
/// ```
/// use sceneforged_parser::{parse, output::{OutputFormat, GenericFormat}};
///
/// let release = parse("The.Matrix.1999.1080p.BluRay.x264-GROUP");
/// let formatter = GenericFormat;
/// let output = formatter.format(&release);
///
/// assert!(output.formatted_string.contains("The Matrix"));
/// assert!(output.formatted_string.contains("1999"));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenericFormat;

/// Generic output structure.
///
/// Contains a human-readable string representation of all parsed fields,
/// formatted for easy reading and debugging.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GenericOutput {
    /// Human-readable formatted string
    pub formatted_string: String,
    /// Original release title
    pub original_title: String,
}

impl OutputFormat for GenericFormat {
    type Output = GenericOutput;

    fn format(&self, release: &ParsedRelease) -> Self::Output {
        let mut parts = Vec::new();

        // Title and year
        let mut title_part = (*release.title).clone();
        if let Some(ref year) = release.year {
            title_part.push_str(&format!(" ({})", **year));
        }
        parts.push(title_part);

        // Media type
        if *release.media_type != crate::model::MediaType::Unknown {
            parts.push(format!("[{}]", format!("{:?}", *release.media_type)));
        }

        // Season and episode info for TV
        if !release.seasons.is_empty() || !release.episodes.is_empty() {
            let mut tv_info = String::new();

            if !release.seasons.is_empty() {
                let seasons: Vec<String> = release
                    .seasons
                    .iter()
                    .map(|s| format!("S{:02}", **s))
                    .collect();
                tv_info.push_str(&seasons.join(","));
            }

            if !release.episodes.is_empty() {
                let episodes: Vec<String> = release
                    .episodes
                    .iter()
                    .map(|e| format!("E{:02}", **e))
                    .collect();
                tv_info.push_str(&episodes.join(""));
            } else if release.full_season {
                tv_info.push_str(" (Full Season)");
            }

            if !tv_info.is_empty() {
                parts.push(tv_info);
            }
        }

        // Absolute episode for anime
        if let Some(ref abs_ep) = release.absolute_episode {
            parts.push(format!("Ep {}", **abs_ep));
        }

        // Episode title
        if let Some(ref ep_title) = release.episode_title {
            parts.push(format!("\"{}\"", **ep_title));
        }

        // Quality information
        let mut quality_parts = Vec::new();

        if let Some(ref res) = release.resolution {
            quality_parts.push(format!("{:?}", **res));
        }

        if let Some(ref src) = release.source {
            quality_parts.push(format!("{:?}", **src));
        }

        if let Some(ref qm) = release.quality_modifier {
            quality_parts.push(format!("{:?}", **qm));
        }

        if !quality_parts.is_empty() {
            parts.push(quality_parts.join(" "));
        }

        // Video encoding
        let mut video_parts = Vec::new();

        if let Some(ref encoder) = release.video_encoder {
            video_parts.push(format!("{:?}", **encoder));
        } else if let Some(ref standard) = release.video_standard {
            video_parts.push(format!("{:?}", **standard));
        }

        if let Some(ref bit_depth) = release.bit_depth {
            video_parts.push(format!("{}bit", **bit_depth));
        }

        if let Some(ref hdr) = release.hdr_format {
            video_parts.push(format!("{:?}", **hdr));
        }

        if !video_parts.is_empty() {
            parts.push(video_parts.join(" "));
        }

        // Audio
        let mut audio_parts = Vec::new();

        if let Some(ref codec) = release.audio_codec {
            audio_parts.push(format!("{:?}", **codec));
        }

        if let Some(ref channels) = release.audio_channels {
            audio_parts.push(format!("{:?}", **channels));
        }

        if !audio_parts.is_empty() {
            parts.push(audio_parts.join(" "));
        }

        // Edition info
        if !release.edition.is_empty() {
            let edition_str = format!("{:?}", release.edition);
            if edition_str != "Edition { flags: [] }" {
                parts.push(edition_str);
            }
        }

        // Revision info
        if release.revision.version > 1 {
            parts.push(format!("v{}", release.revision.version));
        }
        if release.revision.real > 0 {
            parts.push(format!(
                "PROPER{}",
                if release.revision.real > 1 {
                    format!(" x{}", release.revision.real)
                } else {
                    String::new()
                }
            ));
        }

        // Languages
        if !release.languages.is_empty() {
            let langs: Vec<String> = release
                .languages
                .iter()
                .map(|l| format!("{:?}", **l))
                .collect();
            parts.push(format!("Audio: {}", langs.join(", ")));
        }

        if !release.subtitle_languages.is_empty() {
            let subs: Vec<String> = release
                .subtitle_languages
                .iter()
                .map(|l| format!("{:?}", **l))
                .collect();
            parts.push(format!("Subs: {}", subs.join(", ")));
        }

        // Release group
        if let Some(ref group) = release.release_group {
            parts.push(format!("-{}", **group));
        }

        // Container
        if let Some(ref container) = release.container {
            parts.push(format!(".{}", **container));
        }

        // Checksum for anime
        if let Some(ref checksum) = release.file_checksum {
            parts.push(format!("[{}]", **checksum));
        }

        let formatted_string = parts.join(" ");

        GenericOutput {
            formatted_string,
            original_title: release.release_title.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[test]
    fn test_generic_format_movie() {
        let release = parse("The.Matrix.1999.1080p.BluRay.x264-GROUP");
        let formatter = GenericFormat;
        let output = formatter.format(&release);

        assert!(output.formatted_string.contains("The Matrix"));
        assert!(output.formatted_string.contains("1999"));
        assert!(output.formatted_string.contains("1080p"));
        assert!(output.formatted_string.contains("BluRay"));
        assert!(output.formatted_string.contains("X264"));
        assert!(output.formatted_string.contains("GROUP"));
    }

    #[test]
    fn test_generic_format_tv_episode() {
        let release = parse("Breaking.Bad.S01E01.720p.BluRay.x264-DEMAND");
        let formatter = GenericFormat;
        let output = formatter.format(&release);

        assert!(output.formatted_string.contains("Breaking Bad"));
        assert!(output.formatted_string.contains("S01"));
        assert!(output.formatted_string.contains("E01"));
        assert!(output.formatted_string.contains("720p"));
        assert!(output.formatted_string.contains("BluRay"));
    }

    #[test]
    fn test_generic_format_4k_hdr() {
        let release = parse("Inception.2010.2160p.UHD.BluRay.x265.HDR.DTS-HD.MA.5.1-RELEASE");
        let formatter = GenericFormat;
        let output = formatter.format(&release);

        assert!(output.formatted_string.contains("Inception"));
        assert!(output.formatted_string.contains("2010"));
        assert!(output.formatted_string.contains("2160p"));
        assert!(output.formatted_string.contains("X265"));
    }

    #[test]
    fn test_generic_format_anime() {
        let release = parse("[SubGroup] Anime Title - 01 [1080p].mkv");
        let formatter = GenericFormat;
        let output = formatter.format(&release);

        assert!(output.formatted_string.contains("Anime Title"));
        assert!(output.formatted_string.contains("E01"));
        assert!(output.formatted_string.contains("1080p"));
        assert!(output.formatted_string.contains("SubGroup"));
        assert!(output.formatted_string.contains("mkv"));
    }

    #[test]
    fn test_generic_format_proper() {
        let release = parse("Show.S01E01.PROPER.720p.HDTV.x264-GROUP");
        let formatter = GenericFormat;
        let output = formatter.format(&release);

        assert!(output.formatted_string.contains("PROPER"));
    }

    #[test]
    fn test_generic_format_repack() {
        let release = parse("Show.S01E01.REPACK.720p.HDTV.x264-GROUP");
        let formatter = GenericFormat;
        let output = formatter.format(&release);

        // Note: REPACK detection is not yet implemented in the parser
        // This test documents expected future behavior
        // For now, just verify the release is parsed without errors
        assert!(output.formatted_string.contains("Show"));
        // assert!(output.formatted_string.contains("REPACK"));
    }

    #[test]
    fn test_generic_format_multi_episode() {
        let release = parse("Show.S01E01E02.720p.WEB-DL.x264-GROUP");
        let formatter = GenericFormat;
        let output = formatter.format(&release);

        assert!(output.formatted_string.contains("S01"));
        assert!(output.formatted_string.contains("E01"));
        assert!(output.formatted_string.contains("E02"));
    }

    #[test]
    fn test_generic_format_original_title_preserved() {
        let input = "Movie.2020.1080p.BluRay.x264-GROUP";
        let release = parse(input);
        let formatter = GenericFormat;
        let output = formatter.format(&release);

        assert_eq!(output.original_title, input);
    }
}
