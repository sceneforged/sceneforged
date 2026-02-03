//! Trait definition and types for metadata providers.
//!
//! This module defines the [`MetadataProvider`] trait that all metadata backends
//! (TMDB, TVDb, OMDb, Fanart.tv, etc.) must implement, along with the shared
//! data types returned by provider queries.

use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Search results
// ---------------------------------------------------------------------------

/// A single result returned from a metadata search query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Provider-specific identifier for this item (e.g. TMDB numeric ID).
    pub id: String,
    /// Display title of the item.
    pub title: String,
    /// Release or premiere year, if known.
    pub year: Option<u16>,
    /// Short synopsis / overview text.
    pub overview: Option<String>,
    /// How confident the provider is that this result matches the query (0.0 - 1.0).
    pub confidence: f64,
    /// Name of the provider that returned this result (e.g. "tmdb").
    pub provider_name: String,
    /// URL or path fragment for the poster image, if available.
    pub poster_path: Option<String>,
}

// ---------------------------------------------------------------------------
// Metadata
// ---------------------------------------------------------------------------

/// Rich metadata for a movie or TV show.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaMetadata {
    /// Localised title.
    pub title: String,
    /// Original-language title, if different from `title`.
    pub original_title: Option<String>,
    /// Synopsis / overview text.
    pub overview: Option<String>,
    /// Genre labels (e.g. "Action", "Drama").
    pub genres: Vec<String>,
    /// Year the media was first released or premiered.
    pub production_year: Option<u16>,
    /// Exact premiere / release date as an ISO-8601 string (YYYY-MM-DD).
    pub premiere_date: Option<String>,
    /// Community / audience rating (typically 0.0 - 10.0).
    pub community_rating: Option<f64>,
    /// Runtime in minutes, if known.
    pub runtime_minutes: Option<u32>,
    /// Map of external provider IDs keyed by provider name
    /// (e.g. `{"tmdb": "12345", "imdb": "tt1234567"}`).
    pub provider_ids: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Images
// ---------------------------------------------------------------------------

/// Collection of images associated with a movie or TV show.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaImages {
    /// Poster artwork.
    pub posters: Vec<ImageInfo>,
    /// Backdrop / fanart images.
    pub backdrops: Vec<ImageInfo>,
    /// Logo / clear-logo images.
    pub logos: Vec<ImageInfo>,
}

/// A single image with sizing and quality metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    /// Fully-qualified URL to the image.
    pub url: String,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// ISO-639-1 language code for the image content, if applicable.
    pub language: Option<String>,
    /// Community vote average for this image (higher is better).
    pub vote_average: f64,
}

// ---------------------------------------------------------------------------
// Provider trait
// ---------------------------------------------------------------------------

/// Async trait that all metadata providers must implement.
///
/// Each provider wraps a single external API (TMDB, TVDb, OMDb, Fanart.tv,
/// etc.) and exposes a uniform interface for searching and fetching metadata
/// and artwork.
///
/// Providers are expected to be cheaply cloneable or wrapped in an `Arc` so
/// they can be shared across tasks.
#[async_trait]
pub trait MetadataProvider: Send + Sync {
    /// Short, lowercase identifier for this provider (e.g. `"tmdb"`).
    fn name(&self) -> &'static str;

    /// Returns `true` when the provider has been configured with valid
    /// credentials and is ready to serve requests.
    fn is_available(&self) -> bool;

    /// Search for movies matching `title`, optionally constrained by `year`.
    ///
    /// Results are sorted by descending `confidence`.
    async fn search_movie(
        &self,
        title: &str,
        year: Option<u16>,
    ) -> anyhow::Result<Vec<SearchResult>>;

    /// Search for TV shows matching `title`.
    ///
    /// Results are sorted by descending `confidence`.
    async fn search_tv(&self, title: &str) -> anyhow::Result<Vec<SearchResult>>;

    /// Fetch full metadata for a movie identified by `provider_id`.
    async fn get_movie_metadata(&self, provider_id: &str) -> anyhow::Result<MediaMetadata>;

    /// Fetch full metadata for a TV show identified by `provider_id`.
    async fn get_tv_metadata(&self, provider_id: &str) -> anyhow::Result<MediaMetadata>;

    /// Fetch available artwork for a movie identified by `provider_id`.
    async fn get_movie_images(&self, provider_id: &str) -> anyhow::Result<MediaImages>;

    /// Fetch available artwork for a TV show identified by `provider_id`.
    async fn get_tv_images(&self, provider_id: &str) -> anyhow::Result<MediaImages>;
}
