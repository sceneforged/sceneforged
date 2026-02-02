//! Core type definitions for media items, libraries, and streams.
//!
//! This module defines enums used throughout sceneforged for categorizing
//! libraries, items, images, and streams. All enums are serialized in lowercase
//! for compatibility with Jellyfin API expectations.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Type of media library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    /// Movies library containing film content.
    Movies,
    /// TV shows library containing series and episodes.
    TvShows,
    /// Music library containing audio content.
    Music,
}

impl fmt::Display for MediaType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Movies => write!(f, "movies"),
            Self::TvShows => write!(f, "tvshows"),
            Self::Music => write!(f, "music"),
        }
    }
}

/// Kind of library item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    /// A single movie.
    Movie,
    /// A TV series (show).
    Series,
    /// A season within a series.
    Season,
    /// A single episode within a season.
    Episode,
    /// A collection folder grouping multiple items.
    CollectionFolder,
    /// A music album.
    MusicAlbum,
    /// A music artist.
    MusicArtist,
    /// An audio track.
    Audio,
}

impl fmt::Display for ItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Movie => write!(f, "movie"),
            Self::Series => write!(f, "series"),
            Self::Season => write!(f, "season"),
            Self::Episode => write!(f, "episode"),
            Self::CollectionFolder => write!(f, "collectionfolder"),
            Self::MusicAlbum => write!(f, "musicalbum"),
            Self::MusicArtist => write!(f, "musicartist"),
            Self::Audio => write!(f, "audio"),
        }
    }
}

/// Type of item image/artwork.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageType {
    /// Primary poster/thumbnail image.
    Primary,
    /// Background/backdrop image.
    Backdrop,
    /// Thumbnail image.
    Thumb,
    /// Logo image.
    Logo,
    /// Banner image.
    Banner,
    /// Box art image.
    Art,
    /// Disc art image.
    Disc,
}

impl fmt::Display for ImageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Primary => write!(f, "primary"),
            Self::Backdrop => write!(f, "backdrop"),
            Self::Thumb => write!(f, "thumb"),
            Self::Logo => write!(f, "logo"),
            Self::Banner => write!(f, "banner"),
            Self::Art => write!(f, "art"),
            Self::Disc => write!(f, "disc"),
        }
    }
}

/// Type of media stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamType {
    /// Video stream.
    Video,
    /// Audio stream.
    Audio,
    /// Subtitle stream.
    Subtitle,
}

impl fmt::Display for StreamType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Video => write!(f, "video"),
            Self::Audio => write!(f, "audio"),
            Self::Subtitle => write!(f, "subtitle"),
        }
    }
}

/// Role of a media file for an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileRole {
    /// Original source file (MKV remux, download, etc.).
    Source,
    /// Universal playback file (Profile B MP4).
    Universal,
    /// Extra content (trailer, featurette, etc.).
    Extra,
}

impl fmt::Display for FileRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source => write!(f, "source"),
            Self::Universal => write!(f, "universal"),
            Self::Extra => write!(f, "extra"),
        }
    }
}

impl std::str::FromStr for FileRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "source" => Ok(Self::Source),
            "universal" => Ok(Self::Universal),
            "extra" => Ok(Self::Extra),
            _ => Err(format!("Invalid file role: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_type_serialization() {
        let mt = MediaType::Movies;
        let json = serde_json::to_string(&mt).unwrap();
        assert_eq!(json, r#""movies""#);

        let mt = MediaType::TvShows;
        let json = serde_json::to_string(&mt).unwrap();
        assert_eq!(json, r#""tvshows""#);
    }

    #[test]
    fn test_media_type_deserialization() {
        let json = r#""movies""#;
        let mt: MediaType = serde_json::from_str(json).unwrap();
        assert_eq!(mt, MediaType::Movies);

        let json = r#""music""#;
        let mt: MediaType = serde_json::from_str(json).unwrap();
        assert_eq!(mt, MediaType::Music);
    }

    #[test]
    fn test_item_kind_display() {
        assert_eq!(ItemKind::Movie.to_string(), "movie");
        assert_eq!(ItemKind::Series.to_string(), "series");
        assert_eq!(ItemKind::Episode.to_string(), "episode");
        assert_eq!(ItemKind::CollectionFolder.to_string(), "collectionfolder");
    }

    #[test]
    fn test_image_type_serialization() {
        let it = ImageType::Primary;
        let json = serde_json::to_string(&it).unwrap();
        assert_eq!(json, r#""primary""#);

        let it = ImageType::Backdrop;
        let json = serde_json::to_string(&it).unwrap();
        assert_eq!(json, r#""backdrop""#);
    }

    #[test]
    fn test_stream_type_display() {
        assert_eq!(StreamType::Video.to_string(), "video");
        assert_eq!(StreamType::Audio.to_string(), "audio");
        assert_eq!(StreamType::Subtitle.to_string(), "subtitle");
    }

    #[test]
    fn test_stream_type_serialization() {
        let st = StreamType::Video;
        let json = serde_json::to_string(&st).unwrap();
        assert_eq!(json, r#""video""#);

        let deserialized: StreamType = serde_json::from_str(&json).unwrap();
        assert_eq!(st, deserialized);
    }

    #[test]
    fn test_file_role_serialization() {
        let fr = FileRole::Source;
        let json = serde_json::to_string(&fr).unwrap();
        assert_eq!(json, r#""source""#);

        let fr = FileRole::Universal;
        let json = serde_json::to_string(&fr).unwrap();
        assert_eq!(json, r#""universal""#);
    }

    #[test]
    fn test_file_role_display() {
        assert_eq!(FileRole::Source.to_string(), "source");
        assert_eq!(FileRole::Universal.to_string(), "universal");
        assert_eq!(FileRole::Extra.to_string(), "extra");
    }

    #[test]
    fn test_enum_equality() {
        assert_eq!(MediaType::Movies, MediaType::Movies);
        assert_ne!(MediaType::Movies, MediaType::Music);

        assert_eq!(ItemKind::Movie, ItemKind::Movie);
        assert_ne!(ItemKind::Movie, ItemKind::Episode);
    }

    #[test]
    fn test_enum_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(MediaType::Movies);
        set.insert(MediaType::Music);
        assert!(set.contains(&MediaType::Movies));
        assert!(set.contains(&MediaType::Music));
        assert!(!set.contains(&MediaType::TvShows));
    }

    #[test]
    fn test_enum_clone() {
        let mt = MediaType::Movies;
        let cloned = mt;
        assert_eq!(mt, cloned);
    }
}
