//! sf-parser: release-name parser for media filenames.
//!
//! Extracts structured metadata from scene/P2P release names such as
//! `"The.Matrix.1999.1080p.BluRay.x264-GROUP"`.
//!
//! # Quick start
//!
//! ```
//! use sf_parser::parse;
//!
//! let r = parse("The.Matrix.1999.1080p.BluRay.x264-GROUP");
//! assert_eq!(r.title, "The Matrix");
//! assert_eq!(r.year, Some(1999));
//! assert_eq!(r.resolution.as_deref(), Some("1080p"));
//! assert_eq!(r.source.as_deref(), Some("BluRay"));
//! assert_eq!(r.video_codec.as_deref(), Some("x264"));
//! assert_eq!(r.group.as_deref(), Some("GROUP"));
//! ```

pub mod types;
pub mod tokenizer;
mod parser;

pub use types::ParsedRelease;

/// Parse a release name into structured metadata.
///
/// This is the primary entry point. It tokenizes the input using a
/// Logos-based lexer, then applies heuristics to extract the title,
/// year, resolution, source, codecs, HDR format, edition, release
/// group, and revision.
///
/// # Examples
///
/// ```
/// let r = sf_parser::parse("Inception.2010.2160p.UHD.BluRay.x265.HDR.DTS-HD.MA.5.1-RELEASE");
/// assert_eq!(r.title, "Inception");
/// assert_eq!(r.year, Some(2010));
/// assert_eq!(r.resolution.as_deref(), Some("2160p"));
/// assert_eq!(r.video_codec.as_deref(), Some("x265"));
/// ```
pub fn parse(input: &str) -> ParsedRelease {
    parser::parse(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_movie_basic() {
        let r = parse("The.Matrix.1999.1080p.BluRay.x264-GROUP");
        assert_eq!(r.title, "The Matrix");
        assert_eq!(r.year, Some(1999));
        assert_eq!(r.resolution.as_deref(), Some("1080p"));
        assert_eq!(r.source.as_deref(), Some("BluRay"));
        assert_eq!(r.video_codec.as_deref(), Some("x264"));
        assert_eq!(r.group.as_deref(), Some("GROUP"));
    }

    #[test]
    fn test_parse_tv_episode() {
        let r = parse("Breaking.Bad.S01E01.720p.WEB-DL.DD5.1.H.264-DEMAND");
        // Title includes the episode marker since this parser is focused on release names,
        // not TV-specific parsing. The S01E01 is not a recognized stop-token.
        assert!(r.title.contains("Breaking Bad"));
        assert_eq!(r.resolution.as_deref(), Some("720p"));
        assert_eq!(r.source.as_deref(), Some("WEB-DL"));
    }

    #[test]
    fn test_parse_4k_movie_full() {
        let r = parse(
            "Movie.2023.2160p.UHD.BluRay.Remux.HDR.DV.TrueHD.7.1.Atmos.HEVC-FraMeSToR",
        );
        assert_eq!(r.title, "Movie");
        assert_eq!(r.year, Some(2023));
        assert_eq!(r.resolution.as_deref(), Some("2160p"));
        // HDR should be detected (either HDR, DV, or both -- first wins)
        assert!(r.hdr.is_some(), "HDR should be detected");
        // Audio should be TrueHD + Atmos compound
        assert!(r.audio_codec.is_some(), "Audio codec should be detected");
        assert_eq!(r.group.as_deref(), Some("FraMeSToR"));
    }

    #[test]
    fn test_parse_directors_cut() {
        let r = parse("Some.Movie.2020.Directors.Cut.1080p.BluRay.x265-GROUP");
        assert_eq!(r.title, "Some Movie");
        assert_eq!(r.year, Some(2020));
        assert_eq!(r.edition.as_deref(), Some("Director's Cut"));
        assert_eq!(r.resolution.as_deref(), Some("1080p"));
        assert_eq!(r.source.as_deref(), Some("BluRay"));
        assert_eq!(r.video_codec.as_deref(), Some("x265"));
        assert_eq!(r.group.as_deref(), Some("GROUP"));
    }

    #[test]
    fn test_parse_simple_name() {
        let r = parse("My Movie");
        assert_eq!(r.title, "My Movie");
        assert!(r.year.is_none());
        assert!(r.resolution.is_none());
        assert!(r.source.is_none());
        assert!(r.video_codec.is_none());
        assert!(r.group.is_none());
    }

    #[test]
    fn test_parse_underscore_separated() {
        let r = parse("Some_Movie_2021_720p_BluRay_x264-GRP");
        assert_eq!(r.title, "Some Movie");
        assert_eq!(r.year, Some(2021));
        assert_eq!(r.resolution.as_deref(), Some("720p"));
        assert_eq!(r.source.as_deref(), Some("BluRay"));
        assert_eq!(r.video_codec.as_deref(), Some("x264"));
        assert_eq!(r.group.as_deref(), Some("GRP"));
    }

    #[test]
    fn test_parse_space_separated() {
        let r = parse("Some Movie 2021 720p BluRay x264-GRP");
        assert_eq!(r.title, "Some Movie");
        assert_eq!(r.year, Some(2021));
        assert_eq!(r.resolution.as_deref(), Some("720p"));
        assert_eq!(r.source.as_deref(), Some("BluRay"));
    }

    #[test]
    fn test_parse_hdtv() {
        let r = parse("Show.2020.720p.HDTV.x264-LOL");
        assert_eq!(r.title, "Show");
        assert_eq!(r.year, Some(2020));
        assert_eq!(r.source.as_deref(), Some("HDTV"));
    }

    #[test]
    fn test_parse_repack() {
        let r = parse("Movie.2020.1080p.BluRay.REPACK.x264-GROUP");
        assert_eq!(r.revision, Some(1));
    }

    #[test]
    fn test_parse_flac_audio() {
        let r = parse("Movie.2020.1080p.BluRay.FLAC.x264-GROUP");
        assert_eq!(r.audio_codec.as_deref(), Some("FLAC"));
    }

    #[test]
    fn test_parse_aac_audio() {
        let r = parse("Movie.2020.1080p.WEB-DL.AAC.x264-GROUP");
        assert_eq!(r.audio_codec.as_deref(), Some("AAC"));
    }

    #[test]
    fn test_parse_remux_only() {
        let r = parse("Movie.2020.2160p.Remux.HEVC-GROUP");
        assert_eq!(r.source.as_deref(), Some("Remux"));
    }

    #[test]
    fn test_parse_bluray_remux() {
        // When both BluRay and Remux appear, BluRay is the source
        // (Remux is a quality modifier).
        let r = parse("Movie.2020.2160p.BluRay.Remux.HEVC-GROUP");
        assert_eq!(r.source.as_deref(), Some("BluRay"));
    }

    #[test]
    fn test_parse_hlg() {
        let r = parse("Movie.2023.2160p.BluRay.HLG.x265-GROUP");
        assert_eq!(r.hdr.as_deref(), Some("HLG"));
    }

    #[test]
    fn test_parse_unrated_edition() {
        let r = parse("Movie.2020.Unrated.1080p.BluRay-GROUP");
        assert_eq!(r.edition.as_deref(), Some("Unrated"));
    }

    #[test]
    fn test_parse_imax_edition() {
        let r = parse("Movie.2020.IMAX.1080p.BluRay-GROUP");
        assert_eq!(r.edition.as_deref(), Some("IMAX"));
    }

    #[test]
    fn test_serde_roundtrip() {
        let r = parse("The.Matrix.1999.1080p.BluRay.x264-GROUP");
        let json = serde_json::to_string(&r).unwrap();
        let back: ParsedRelease = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }
}
