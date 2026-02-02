//! Main parsed release structure.

use super::{
    AudioChannels, AudioCodec, Confidence, Edition, FrameRate, HdrFormat, Language, MediaType,
    OptionalField, ParsedField, QualityModifier, Resolution, Revision, Source, StreamingService,
    VideoEncoder, VideoStandard,
};

/// The main output type containing all parsed metadata from a release name.
///
/// This struct represents all the information that can be extracted from a
/// media release name, including title, year, quality, codecs, and more.
///
/// Fields are wrapped in `ParsedField<T>` to track confidence, source span,
/// and alternative interpretations.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParsedRelease {
    /// The extracted title of the media
    pub title: ParsedField<String>,
    /// Release year
    pub year: OptionalField<u16>,
    /// Type of media (Movie, TV, Anime, Unknown)
    pub media_type: ParsedField<MediaType>,

    // TV/Anime episode info
    /// Season numbers (can be multiple for season packs)
    pub seasons: Vec<ParsedField<u16>>,
    /// Episode numbers (can be multiple for multi-episode releases)
    pub episodes: Vec<ParsedField<u16>>,
    /// Episode title if present
    pub episode_title: OptionalField<String>,
    /// Absolute episode number (common in anime)
    pub absolute_episode: OptionalField<u16>,
    /// Air date of the episode
    #[cfg(feature = "chrono")]
    pub air_date: OptionalField<chrono::NaiveDate>,
    /// Whether this is a full season release
    pub full_season: bool,
    /// Whether this is a special/OVA episode
    pub is_special: bool,

    // Quality
    /// Video resolution (720p, 1080p, 2160p, etc.)
    pub resolution: OptionalField<Resolution>,
    /// Source of the release (BluRay, WEB-DL, etc.)
    pub source: OptionalField<Source>,
    /// Quality modifier (Remux, BrDisk, etc.)
    pub quality_modifier: OptionalField<QualityModifier>,

    // Encoding
    /// Video compression standard (H.264, H.265, AV1, etc.)
    pub video_standard: OptionalField<VideoStandard>,
    /// Video encoder implementation (x264, x265, SVT-AV1, etc.)
    pub video_encoder: OptionalField<VideoEncoder>,
    /// Audio codec (DTS-HD MA, TrueHD, AAC, etc.)
    pub audio_codec: OptionalField<AudioCodec>,
    /// Audio channel configuration (5.1, 7.1, etc.)
    pub audio_channels: OptionalField<AudioChannels>,
    /// Video bit depth (8, 10, 12)
    pub bit_depth: OptionalField<u8>,
    /// HDR format if present
    pub hdr_format: OptionalField<HdrFormat>,
    /// Frame rate if specified
    pub frame_rate: OptionalField<FrameRate>,

    // Release metadata
    /// Release group name
    pub release_group: OptionalField<String>,
    /// Revision information (version, PROPER/REAL count)
    pub revision: Revision,
    /// Edition flags (Director's Cut, Extended, etc.)
    pub edition: Edition,
    /// Streaming service origin
    pub streaming_service: OptionalField<StreamingService>,

    // Localisation
    /// Audio languages
    pub languages: Vec<ParsedField<Language>>,
    /// Subtitle languages
    pub subtitle_languages: Vec<ParsedField<Language>>,

    // File info
    /// Container format (mkv, mp4, avi)
    pub container: OptionalField<String>,
    /// File checksum (CRC32 from filename)
    pub file_checksum: OptionalField<String>,

    // Raw
    /// Original release title as provided
    pub release_title: String,
}

impl ParsedRelease {
    /// Create a new ParsedRelease with the given input string.
    ///
    /// All fields are initialized to their default values except for
    /// `release_title` which is set to the input, `title` which is set to
    /// an empty string with medium confidence, and `media_type` which is
    /// set to `MediaType::Unknown` with medium confidence.
    pub fn new(input: impl Into<String>) -> Self {
        let input_str = input.into();
        let input_len = input_str.len();

        Self {
            release_title: input_str,
            title: ParsedField::new(String::new(), Confidence::MEDIUM, (0, 0), ""),
            media_type: ParsedField::new(
                MediaType::Unknown,
                Confidence::MEDIUM,
                (0, input_len),
                "",
            ),
            year: None,
            seasons: Vec::new(),
            episodes: Vec::new(),
            episode_title: None,
            absolute_episode: None,
            #[cfg(feature = "chrono")]
            air_date: None,
            full_season: false,
            is_special: false,
            resolution: None,
            source: None,
            quality_modifier: None,
            video_standard: None,
            video_encoder: None,
            audio_codec: None,
            audio_channels: None,
            bit_depth: None,
            hdr_format: None,
            frame_rate: None,
            release_group: None,
            revision: Revision::default(),
            edition: Edition::default(),
            streaming_service: None,
            languages: Vec::new(),
            subtitle_languages: Vec::new(),
            container: None,
            file_checksum: None,
        }
    }

    /// Calculate overall confidence across all fields.
    ///
    /// Returns the minimum confidence score found across all parsed fields.
    /// Fields with no value (None) are not considered in the calculation.
    pub fn overall_confidence(&self) -> Confidence {
        let mut min_value = Confidence::CERTAIN.value();

        // Helper to update min
        let update_min = |current: f32, conf: Confidence| current.min(conf.value());

        // Check required fields
        min_value = update_min(min_value, self.title.confidence);
        min_value = update_min(min_value, self.media_type.confidence);

        // Check optional fields
        if let Some(ref field) = self.year {
            min_value = update_min(min_value, field.confidence);
        }
        if let Some(ref field) = self.resolution {
            min_value = update_min(min_value, field.confidence);
        }
        if let Some(ref field) = self.source {
            min_value = update_min(min_value, field.confidence);
        }
        if let Some(ref field) = self.quality_modifier {
            min_value = update_min(min_value, field.confidence);
        }
        if let Some(ref field) = self.video_standard {
            min_value = update_min(min_value, field.confidence);
        }
        if let Some(ref field) = self.video_encoder {
            min_value = update_min(min_value, field.confidence);
        }
        if let Some(ref field) = self.audio_codec {
            min_value = update_min(min_value, field.confidence);
        }
        if let Some(ref field) = self.audio_channels {
            min_value = update_min(min_value, field.confidence);
        }
        if let Some(ref field) = self.bit_depth {
            min_value = update_min(min_value, field.confidence);
        }
        if let Some(ref field) = self.hdr_format {
            min_value = update_min(min_value, field.confidence);
        }
        if let Some(ref field) = self.frame_rate {
            min_value = update_min(min_value, field.confidence);
        }
        if let Some(ref field) = self.release_group {
            min_value = update_min(min_value, field.confidence);
        }
        if let Some(ref field) = self.streaming_service {
            min_value = update_min(min_value, field.confidence);
        }
        if let Some(ref field) = self.episode_title {
            min_value = update_min(min_value, field.confidence);
        }
        if let Some(ref field) = self.absolute_episode {
            min_value = update_min(min_value, field.confidence);
        }
        #[cfg(feature = "chrono")]
        if let Some(ref field) = self.air_date {
            min_value = update_min(min_value, field.confidence);
        }
        if let Some(ref field) = self.container {
            min_value = update_min(min_value, field.confidence);
        }
        if let Some(ref field) = self.file_checksum {
            min_value = update_min(min_value, field.confidence);
        }

        // Check Vec fields
        for field in &self.seasons {
            min_value = update_min(min_value, field.confidence);
        }
        for field in &self.episodes {
            min_value = update_min(min_value, field.confidence);
        }
        for field in &self.languages {
            min_value = update_min(min_value, field.confidence);
        }
        for field in &self.subtitle_languages {
            min_value = update_min(min_value, field.confidence);
        }

        Confidence::new(min_value)
    }

    /// Returns a list of field names that need review (confidence < 0.5).
    ///
    /// Fields with low confidence should be validated against external sources
    /// or reviewed by a user.
    pub fn fields_needing_review(&self) -> Vec<&'static str> {
        let mut needs_review = Vec::new();

        if self.title.confidence.needs_review() {
            needs_review.push("title");
        }
        if self.media_type.confidence.needs_review() {
            needs_review.push("media_type");
        }
        if let Some(ref field) = self.year {
            if field.confidence.needs_review() {
                needs_review.push("year");
            }
        }
        if let Some(ref field) = self.resolution {
            if field.confidence.needs_review() {
                needs_review.push("resolution");
            }
        }
        if let Some(ref field) = self.source {
            if field.confidence.needs_review() {
                needs_review.push("source");
            }
        }
        if let Some(ref field) = self.video_standard {
            if field.confidence.needs_review() {
                needs_review.push("video_standard");
            }
        }
        if let Some(ref field) = self.video_encoder {
            if field.confidence.needs_review() {
                needs_review.push("video_encoder");
            }
        }
        if let Some(ref field) = self.audio_codec {
            if field.confidence.needs_review() {
                needs_review.push("audio_codec");
            }
        }
        if let Some(ref field) = self.release_group {
            if field.confidence.needs_review() {
                needs_review.push("release_group");
            }
        }

        // Check Vec fields
        if self.seasons.iter().any(|f| f.confidence.needs_review()) {
            needs_review.push("seasons");
        }
        if self.episodes.iter().any(|f| f.confidence.needs_review()) {
            needs_review.push("episodes");
        }

        needs_review
    }

    /// Returns true if this release appears to be a TV series episode.
    pub fn is_tv(&self) -> bool {
        matches!(*self.media_type, MediaType::Tv | MediaType::Anime)
            || !self.seasons.is_empty()
            || !self.episodes.is_empty()
            || self.absolute_episode.is_some()
    }

    /// Returns true if this release appears to be a movie.
    pub fn is_movie(&self) -> bool {
        matches!(*self.media_type, MediaType::Movie)
            || (*self.media_type == MediaType::Unknown
                && self.seasons.is_empty()
                && self.episodes.is_empty()
                && self.absolute_episode.is_none()
                && self.year.is_some())
    }
}

impl Default for ParsedRelease {
    fn default() -> Self {
        Self {
            title: ParsedField::new(String::new(), Confidence::MEDIUM, (0, 0), ""),
            year: None,
            media_type: ParsedField::new(MediaType::Unknown, Confidence::MEDIUM, (0, 0), ""),
            seasons: Vec::new(),
            episodes: Vec::new(),
            episode_title: None,
            absolute_episode: None,
            #[cfg(feature = "chrono")]
            air_date: None,
            full_season: false,
            is_special: false,
            resolution: None,
            source: None,
            quality_modifier: None,
            video_standard: None,
            video_encoder: None,
            audio_codec: None,
            audio_channels: None,
            bit_depth: None,
            hdr_format: None,
            frame_rate: None,
            release_group: None,
            revision: Revision::default(),
            edition: Edition::default(),
            streaming_service: None,
            languages: Vec::new(),
            subtitle_languages: Vec::new(),
            container: None,
            file_checksum: None,
            release_title: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsed_release_new() {
        let release = ParsedRelease::new("Test.Release.2024.1080p.BluRay.x264-GROUP");
        assert_eq!(
            release.release_title,
            "Test.Release.2024.1080p.BluRay.x264-GROUP"
        );
        assert_eq!(*release.media_type, MediaType::Unknown);
        assert_eq!(*release.title, String::new());
    }

    #[test]
    fn parsed_release_default() {
        let release = ParsedRelease::default();
        assert!(release.release_title.is_empty());
        assert_eq!(*release.media_type, MediaType::Unknown);
        assert!(release.seasons.is_empty());
        assert!(release.episodes.is_empty());
        assert_eq!(release.revision.version, 1);
        assert!(release.edition.is_empty());
    }

    #[test]
    fn parsed_release_is_tv() {
        let mut release = ParsedRelease::default();
        assert!(!release.is_tv());

        release.seasons.push(ParsedField::certain(1, (0, 0), "S01"));
        assert!(release.is_tv());

        release.seasons.clear();
        release
            .episodes
            .push(ParsedField::certain(1, (0, 0), "E01"));
        assert!(release.is_tv());

        release.episodes.clear();
        release.media_type = ParsedField::certain(MediaType::Tv, (0, 0), "");
        assert!(release.is_tv());
    }

    #[test]
    fn parsed_release_is_movie() {
        let mut release = ParsedRelease {
            year: Some(ParsedField::certain(2024, (0, 4), "2024")),
            ..Default::default()
        };
        assert!(release.is_movie());

        release.media_type = ParsedField::certain(MediaType::Movie, (0, 0), "");
        assert!(release.is_movie());

        release.seasons.push(ParsedField::certain(1, (0, 0), "S01"));
        release.media_type = ParsedField::certain(MediaType::Unknown, (0, 0), "");
        assert!(!release.is_movie());
    }

    #[test]
    fn overall_confidence_calculation() {
        let mut release = ParsedRelease::default();
        // Default should have MEDIUM confidence for title and media_type
        assert_eq!(release.overall_confidence(), Confidence::MEDIUM);

        // Adding a low confidence field should lower overall
        release.year = Some(ParsedField::new(2024, Confidence::LOW, (0, 4), "2024"));
        assert_eq!(release.overall_confidence(), Confidence::LOW);

        // Adding a GUESS confidence field should lower it further
        release.resolution = Some(ParsedField::new(
            Resolution::_1080p,
            Confidence::GUESS,
            (5, 10),
            "1080p",
        ));
        assert_eq!(release.overall_confidence(), Confidence::GUESS);
    }

    #[test]
    fn fields_needing_review() {
        let mut release = ParsedRelease::default();
        // Default fields have MEDIUM confidence, so no review needed
        assert_eq!(release.fields_needing_review(), Vec::<&str>::new());

        // Add a field with GUESS confidence (needs review)
        release.year = Some(ParsedField::new(2024, Confidence::GUESS, (0, 4), "2024"));
        let needs_review = release.fields_needing_review();
        assert!(needs_review.contains(&"year"));

        // Add another low confidence field
        release.resolution = Some(ParsedField::new(
            Resolution::_1080p,
            Confidence::GUESS,
            (5, 10),
            "1080p",
        ));
        let needs_review = release.fields_needing_review();
        assert!(needs_review.contains(&"year"));
        assert!(needs_review.contains(&"resolution"));
    }
}
