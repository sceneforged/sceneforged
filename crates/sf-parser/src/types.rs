//! Output types for the release name parser.

use serde::{Deserialize, Serialize};

/// Structured metadata extracted from a media release filename.
///
/// Fields are populated on a best-effort basis; only `title` is guaranteed
/// to be non-empty. All other fields are `Option` (or `Vec`) and will be
/// `None`/empty when the corresponding token is not found in the input.
///
/// # Examples
///
/// ```
/// use sf_parser::parse;
///
/// let r = parse("The.Matrix.1999.1080p.BluRay.x264-GROUP");
/// assert_eq!(r.title, "The Matrix");
/// assert_eq!(r.year, Some(1999));
/// assert_eq!(r.resolution.as_deref(), Some("1080p"));
/// assert_eq!(r.source.as_deref(), Some("BluRay"));
/// assert_eq!(r.video_codec.as_deref(), Some("x264"));
/// assert_eq!(r.group.as_deref(), Some("GROUP"));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedRelease {
    /// The cleaned title (dots/underscores replaced with spaces, trimmed).
    pub title: String,

    /// Release year (1900--2099).
    pub year: Option<u32>,

    /// Season number (from S01E01-style tags).
    pub season: Option<u32>,

    /// Episode number (from S01E01-style tags).
    pub episode: Option<u32>,

    /// End episode for multi-episode releases (e.g. S01E01E02 → episode_end = 2).
    pub episode_end: Option<u32>,

    /// Video resolution, e.g. `"1080p"`, `"2160p"`, `"720p"`.
    pub resolution: Option<String>,

    /// Media source, e.g. `"BluRay"`, `"WEB-DL"`, `"WEB"`, `"HDTV"`, `"Remux"`.
    pub source: Option<String>,

    /// Video codec, e.g. `"x264"`, `"x265"`, `"H.264"`, `"H.265"`, `"AV1"`.
    pub video_codec: Option<String>,

    /// Audio codec, e.g. `"AAC"`, `"DTS"`, `"DTS-HD"`, `"TrueHD"`, `"Atmos"`, `"FLAC"`, `"EAC3"`, `"AC3"`.
    pub audio_codec: Option<String>,

    /// HDR format, e.g. `"HDR"`, `"HDR10"`, `"HDR10+"`, `"DV"`, `"DoVi"`.
    pub hdr: Option<String>,

    /// Detected languages.
    pub languages: Vec<String>,

    /// Edition, e.g. `"Director's Cut"`, `"Extended"`, `"Unrated"`, `"Remastered"`.
    pub edition: Option<String>,

    /// Release group (usually the text after the final hyphen).
    pub group: Option<String>,

    /// Revision indicator extracted from `"PROPER"` (1), `"REPACK"` (1), `"v2"` (2), `"v3"` (3), etc.
    pub revision: Option<u8>,
}

impl ParsedRelease {
    /// Create a new `ParsedRelease` with only the title populated.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            year: None,
            season: None,
            episode: None,
            episode_end: None,
            resolution: None,
            source: None,
            video_codec: None,
            audio_codec: None,
            hdr: None,
            languages: Vec::new(),
            edition: None,
            group: None,
            revision: None,
        }
    }
}
