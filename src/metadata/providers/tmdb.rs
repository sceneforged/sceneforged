//! TMDB (The Movie Database) metadata provider.
//!
//! Implements [`MetadataProvider`] by querying the TMDB v3 REST API.
//!
//! Features:
//! - Token-bucket rate limiting at 4 requests / second via [`governor`].
//! - Automatic retry on HTTP 429 with `Retry-After` header support (max 3 retries).
//! - 30-second request timeout.
//! - Confidence scoring based on title similarity and year proximity.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::time::Duration;

use anyhow::Context;
use async_trait::async_trait;
use governor::{Quota, RateLimiter};
use reqwest::StatusCode;
use serde::Deserialize;
use tracing::{debug, warn};

use crate::metadata::provider::{
    ImageInfo, MediaImages, MediaMetadata, MetadataProvider, SearchResult,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const TMDB_BASE_URL: &str = "https://api.themoviedb.org/3";
const TMDB_IMAGE_BASE: &str = "https://image.tmdb.org/t/p/original";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RETRIES: u32 = 3;

// ---------------------------------------------------------------------------
// TMDB API response types (private)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct TmdbSearchResponse<T> {
    results: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct TmdbMovieSearchResult {
    id: u64,
    title: Option<String>,
    release_date: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TmdbTvSearchResult {
    id: u64,
    name: Option<String>,
    first_air_date: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TmdbMovieDetail {
    title: Option<String>,
    original_title: Option<String>,
    overview: Option<String>,
    release_date: Option<String>,
    vote_average: Option<f64>,
    runtime: Option<u32>,
    genres: Option<Vec<TmdbGenre>>,
    imdb_id: Option<String>,
    id: u64,
}

#[derive(Debug, Deserialize)]
struct TmdbTvDetail {
    name: Option<String>,
    original_name: Option<String>,
    overview: Option<String>,
    first_air_date: Option<String>,
    vote_average: Option<f64>,
    episode_run_time: Option<Vec<u32>>,
    genres: Option<Vec<TmdbGenre>>,
    id: u64,
    external_ids: Option<TmdbExternalIds>,
}

#[derive(Debug, Deserialize)]
struct TmdbGenre {
    name: String,
}

#[derive(Debug, Deserialize)]
struct TmdbExternalIds {
    imdb_id: Option<String>,
    tvdb_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct TmdbImagesResponse {
    posters: Option<Vec<TmdbImage>>,
    backdrops: Option<Vec<TmdbImage>>,
    logos: Option<Vec<TmdbImage>>,
}

#[derive(Debug, Deserialize)]
struct TmdbImage {
    file_path: String,
    width: u32,
    height: u32,
    iso_639_1: Option<String>,
    vote_average: f64,
}

// ---------------------------------------------------------------------------
// Provider implementation
// ---------------------------------------------------------------------------

/// TMDB metadata provider.
///
/// Wraps the TMDB v3 REST API with built-in rate limiting, retry logic, and
/// confidence-scored search results.
///
/// # Examples
///
/// ```no_run
/// use sceneforged::metadata::providers::TmdbProvider;
///
/// let provider = TmdbProvider::new("your-api-key".into(), "en-US".into());
/// ```
pub struct TmdbProvider {
    client: reqwest::Client,
    api_key: String,
    language: String,
    rate_limiter: governor::RateLimiter<
        governor::state::NotKeyed,
        governor::state::InMemoryState,
        governor::clock::DefaultClock,
    >,
}

impl TmdbProvider {
    /// Create a new TMDB provider with the given API key and language.
    ///
    /// The `language` parameter should be an ISO-639-1 language tag such as
    /// `"en-US"`. Rate limiting is configured at 4 requests per second.
    pub fn new(api_key: String, language: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("failed to build reqwest client");

        let quota = Quota::per_second(NonZeroU32::new(4).unwrap());
        let rate_limiter = RateLimiter::direct(quota);

        Self {
            client,
            api_key,
            language,
            rate_limiter,
        }
    }

    /// Execute a GET request with rate limiting and 429-retry logic.
    async fn get(&self, url: &str) -> anyhow::Result<reqwest::Response> {
        let mut retries = 0u32;
        loop {
            self.rate_limiter.until_ready().await;

            let resp = self
                .client
                .get(url)
                .send()
                .await
                .with_context(|| format!("TMDB request failed: {url}"))?;

            if resp.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES {
                retries += 1;
                let wait = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(1);
                warn!(
                    retry = retries,
                    wait_secs = wait,
                    "TMDB returned 429, backing off"
                );
                tokio::time::sleep(Duration::from_secs(wait)).await;
                continue;
            }

            let resp = resp
                .error_for_status()
                .with_context(|| format!("TMDB request returned error: {url}"))?;

            return Ok(resp);
        }
    }

    /// Build a full API URL with the API key and language query parameters.
    fn url(&self, path: &str, extra_params: &[(&str, &str)]) -> String {
        let mut url = format!(
            "{TMDB_BASE_URL}{path}?api_key={}&language={}",
            self.api_key, self.language
        );
        for (key, value) in extra_params {
            url.push('&');
            url.push_str(key);
            url.push('=');
            url.push_str(&urlencoded(value));
        }
        url
    }

    /// Compute confidence score for a search result based on title similarity
    /// and year proximity.
    fn confidence(
        query_title: &str,
        result_title: &str,
        query_year: Option<u16>,
        result_year: Option<u16>,
    ) -> f64 {
        // Title scoring
        let base = if query_title == result_title {
            0.5
        } else if query_title.eq_ignore_ascii_case(result_title) {
            0.4
        } else if result_title
            .to_ascii_lowercase()
            .contains(&query_title.to_ascii_lowercase())
        {
            0.2
        } else {
            0.1
        };

        // Year scoring
        let year_bonus = match (query_year, result_year) {
            (Some(q), Some(r)) if q == r => 0.3,
            (Some(q), Some(r)) if q.abs_diff(r) <= 1 => 0.15,
            _ => 0.0,
        };

        base + year_bonus
    }
}

/// Minimal percent-encoding for query parameter values.
fn urlencoded(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push(char::from(HEX[(b >> 4) as usize]));
                out.push(char::from(HEX[(b & 0x0f) as usize]));
            }
        }
    }
    out
}

const HEX: [u8; 16] = *b"0123456789ABCDEF";

/// Extract a four-digit year from a date string like `"2023-04-15"`.
fn parse_year(date: &Option<String>) -> Option<u16> {
    date.as_deref()
        .and_then(|d| d.get(..4))
        .and_then(|y| y.parse::<u16>().ok())
}

/// Convert a TMDB image path fragment to a full URL.
fn image_url(path: &str) -> String {
    format!("{TMDB_IMAGE_BASE}{path}")
}

/// Convert a [`TmdbImage`] to an [`ImageInfo`].
fn to_image_info(img: &TmdbImage) -> ImageInfo {
    ImageInfo {
        url: image_url(&img.file_path),
        width: img.width,
        height: img.height,
        language: img.iso_639_1.clone(),
        vote_average: img.vote_average,
    }
}

#[async_trait]
impl MetadataProvider for TmdbProvider {
    fn name(&self) -> &'static str {
        "tmdb"
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    async fn search_movie(
        &self,
        title: &str,
        year: Option<u16>,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let mut params = vec![("query", title)];
        let year_str = year.map(|y| y.to_string());
        if let Some(ref y) = year_str {
            params.push(("year", y.as_str()));
        }

        let url = self.url("/search/movie", &params);
        debug!(url = %url, "TMDB search movie");

        let body: TmdbSearchResponse<TmdbMovieSearchResult> = self
            .get(&url)
            .await?
            .json()
            .await
            .context("failed to parse TMDB movie search response")?;

        let mut results: Vec<SearchResult> = body
            .results
            .into_iter()
            .map(|r| {
                let result_title = r.title.unwrap_or_default();
                let result_year = parse_year(&r.release_date);
                let confidence = Self::confidence(title, &result_title, year, result_year);
                SearchResult {
                    id: r.id.to_string(),
                    title: result_title,
                    year: result_year,
                    overview: r.overview,
                    confidence,
                    provider_name: "tmdb".to_string(),
                    poster_path: r.poster_path.map(|p| image_url(&p)),
                }
            })
            .collect();

        results.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(results)
    }

    async fn search_tv(&self, title: &str) -> anyhow::Result<Vec<SearchResult>> {
        let url = self.url("/search/tv", &[("query", title)]);
        debug!(url = %url, "TMDB search TV");

        let body: TmdbSearchResponse<TmdbTvSearchResult> = self
            .get(&url)
            .await?
            .json()
            .await
            .context("failed to parse TMDB TV search response")?;

        let mut results: Vec<SearchResult> = body
            .results
            .into_iter()
            .map(|r| {
                let result_title = r.name.unwrap_or_default();
                let result_year = parse_year(&r.first_air_date);
                let confidence = Self::confidence(title, &result_title, None, result_year);
                SearchResult {
                    id: r.id.to_string(),
                    title: result_title,
                    year: result_year,
                    overview: r.overview,
                    confidence,
                    provider_name: "tmdb".to_string(),
                    poster_path: r.poster_path.map(|p| image_url(&p)),
                }
            })
            .collect();

        results.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(results)
    }

    async fn get_movie_metadata(&self, provider_id: &str) -> anyhow::Result<MediaMetadata> {
        let url = self.url(&format!("/movie/{provider_id}"), &[]);
        debug!(url = %url, "TMDB get movie metadata");

        let detail: TmdbMovieDetail = self
            .get(&url)
            .await?
            .json()
            .await
            .context("failed to parse TMDB movie detail response")?;

        let mut provider_ids = HashMap::new();
        provider_ids.insert("tmdb".to_string(), detail.id.to_string());
        if let Some(imdb) = detail.imdb_id {
            provider_ids.insert("imdb".to_string(), imdb);
        }

        Ok(MediaMetadata {
            title: detail.title.unwrap_or_default(),
            original_title: detail.original_title,
            overview: detail.overview,
            genres: detail
                .genres
                .unwrap_or_default()
                .into_iter()
                .map(|g| g.name)
                .collect(),
            production_year: parse_year(&detail.release_date),
            premiere_date: detail.release_date,
            community_rating: detail.vote_average,
            runtime_minutes: detail.runtime,
            provider_ids,
        })
    }

    async fn get_tv_metadata(&self, provider_id: &str) -> anyhow::Result<MediaMetadata> {
        let url = self.url(
            &format!("/tv/{provider_id}"),
            &[("append_to_response", "external_ids")],
        );
        debug!(url = %url, "TMDB get TV metadata");

        let detail: TmdbTvDetail = self
            .get(&url)
            .await?
            .json()
            .await
            .context("failed to parse TMDB TV detail response")?;

        let mut provider_ids = HashMap::new();
        provider_ids.insert("tmdb".to_string(), detail.id.to_string());
        if let Some(ref ext) = detail.external_ids {
            if let Some(ref imdb) = ext.imdb_id {
                provider_ids.insert("imdb".to_string(), imdb.clone());
            }
            if let Some(tvdb) = ext.tvdb_id {
                provider_ids.insert("tvdb".to_string(), tvdb.to_string());
            }
        }

        let runtime = detail
            .episode_run_time
            .as_ref()
            .and_then(|v| v.first().copied());

        Ok(MediaMetadata {
            title: detail.name.unwrap_or_default(),
            original_title: detail.original_name,
            overview: detail.overview,
            genres: detail
                .genres
                .unwrap_or_default()
                .into_iter()
                .map(|g| g.name)
                .collect(),
            production_year: parse_year(&detail.first_air_date),
            premiere_date: detail.first_air_date,
            community_rating: detail.vote_average,
            runtime_minutes: runtime,
            provider_ids,
        })
    }

    async fn get_movie_images(&self, provider_id: &str) -> anyhow::Result<MediaImages> {
        let url = self.url(&format!("/movie/{provider_id}/images"), &[]);
        debug!(url = %url, "TMDB get movie images");

        let resp: TmdbImagesResponse = self
            .get(&url)
            .await?
            .json()
            .await
            .context("failed to parse TMDB movie images response")?;

        Ok(MediaImages {
            posters: resp
                .posters
                .unwrap_or_default()
                .iter()
                .map(to_image_info)
                .collect(),
            backdrops: resp
                .backdrops
                .unwrap_or_default()
                .iter()
                .map(to_image_info)
                .collect(),
            logos: resp
                .logos
                .unwrap_or_default()
                .iter()
                .map(to_image_info)
                .collect(),
        })
    }

    async fn get_tv_images(&self, provider_id: &str) -> anyhow::Result<MediaImages> {
        let url = self.url(&format!("/tv/{provider_id}/images"), &[]);
        debug!(url = %url, "TMDB get TV images");

        let resp: TmdbImagesResponse = self
            .get(&url)
            .await?
            .json()
            .await
            .context("failed to parse TMDB TV images response")?;

        Ok(MediaImages {
            posters: resp
                .posters
                .unwrap_or_default()
                .iter()
                .map(to_image_info)
                .collect(),
            backdrops: resp
                .backdrops
                .unwrap_or_default()
                .iter()
                .map(to_image_info)
                .collect(),
            logos: resp
                .logos
                .unwrap_or_default()
                .iter()
                .map(to_image_info)
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_exact_title_match() {
        let score = TmdbProvider::confidence("Inception", "Inception", Some(2010), Some(2010));
        assert!((score - 0.8).abs() < f64::EPSILON); // 0.5 + 0.3
    }

    #[test]
    fn confidence_case_insensitive_match() {
        let score = TmdbProvider::confidence("inception", "Inception", None, None);
        assert!((score - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn confidence_contains_match() {
        let score = TmdbProvider::confidence("Alien", "Aliens", None, None);
        assert!((score - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn confidence_close_year() {
        let score = TmdbProvider::confidence("Dune", "Dune", Some(2021), Some(2020));
        assert!((score - 0.65).abs() < f64::EPSILON); // 0.5 + 0.15
    }

    #[test]
    fn confidence_no_match() {
        let score = TmdbProvider::confidence("Foo", "Bar", None, None);
        assert!((score - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn year_parsing() {
        assert_eq!(parse_year(&Some("2023-04-15".to_string())), Some(2023));
        assert_eq!(parse_year(&Some("1999".to_string())), Some(1999));
        assert_eq!(parse_year(&None), None);
        assert_eq!(parse_year(&Some("".to_string())), None);
    }

    #[test]
    fn image_url_construction() {
        assert_eq!(
            image_url("/abc123.jpg"),
            "https://image.tmdb.org/t/p/original/abc123.jpg"
        );
    }

    #[test]
    fn url_encoding() {
        assert_eq!(urlencoded("hello world"), "hello+world");
        assert_eq!(urlencoded("foo&bar"), "foo%26bar");
        assert_eq!(urlencoded("simple"), "simple");
    }

    #[test]
    fn provider_is_available() {
        let provider = TmdbProvider::new("test-key".into(), "en-US".into());
        assert!(provider.is_available());

        let empty = TmdbProvider::new(String::new(), "en-US".into());
        assert!(!empty.is_available());
    }

    #[test]
    fn provider_name() {
        let provider = TmdbProvider::new("key".into(), "en-US".into());
        assert_eq!(provider.name(), "tmdb");
    }
}
