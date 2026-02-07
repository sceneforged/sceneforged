//! TMDB (The Movie Database) API client.
//!
//! Provides search and detail lookup for movies and TV shows, plus image
//! downloading. Rate-limited to avoid hitting TMDB's API limits.

use std::num::NonZeroU32;
use std::sync::Arc;

use governor::{Quota, RateLimiter};
use serde::{Deserialize, Serialize};

const BASE_URL: &str = "https://api.themoviedb.org/3";
const IMAGE_BASE_URL: &str = "https://image.tmdb.org/t/p";

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

pub struct TmdbClient {
    http: reqwest::Client,
    api_key: String,
    language: String,
    limiter: Arc<RateLimiter<governor::state::NotKeyed, governor::state::InMemoryState, governor::clock::DefaultClock>>,
}

impl TmdbClient {
    pub fn new(api_key: String, language: String) -> Self {
        let quota = Quota::per_second(NonZeroU32::new(30).unwrap());
        let limiter = Arc::new(RateLimiter::direct(quota));
        Self {
            http: reqwest::Client::new(),
            api_key,
            language,
            limiter,
        }
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str, extra_params: &[(&str, &str)]) -> sf_core::Result<T> {
        self.limiter.until_ready().await;

        let url = format!("{BASE_URL}{path}");
        let mut params: Vec<(&str, &str)> = vec![
            ("api_key", &self.api_key),
            ("language", &self.language),
        ];
        params.extend_from_slice(extra_params);

        let resp = self.http.get(&url).query(&params).send().await
            .map_err(|e| sf_core::Error::Internal(format!("TMDB request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(sf_core::Error::Internal(format!("TMDB {status}: {body}")));
        }

        resp.json::<T>().await
            .map_err(|e| sf_core::Error::Internal(format!("TMDB parse error: {e}")))
    }

    // -----------------------------------------------------------------------
    // Search
    // -----------------------------------------------------------------------

    pub async fn search_movie(&self, query: &str, year: Option<u32>) -> sf_core::Result<Vec<TmdbSearchResult>> {
        let mut params: Vec<(&str, &str)> = vec![("query", query)];
        let year_str = year.map(|y| y.to_string());
        if let Some(ref y) = year_str {
            params.push(("year", y.as_str()));
        }
        let resp: TmdbSearchResponse = self.get("/search/movie", &params).await?;
        Ok(resp.results)
    }

    pub async fn search_tv(&self, query: &str, year: Option<u32>) -> sf_core::Result<Vec<TmdbSearchResult>> {
        let mut params: Vec<(&str, &str)> = vec![("query", query)];
        let year_str = year.map(|y| y.to_string());
        if let Some(ref y) = year_str {
            params.push(("first_air_date_year", y.as_str()));
        }
        let resp: TmdbSearchResponse = self.get("/search/tv", &params).await?;
        Ok(resp.results)
    }

    // -----------------------------------------------------------------------
    // Details
    // -----------------------------------------------------------------------

    pub async fn get_movie(&self, id: u64) -> sf_core::Result<TmdbMovie> {
        self.get(&format!("/movie/{id}"), &[]).await
    }

    pub async fn get_tv(&self, id: u64) -> sf_core::Result<TmdbTvShow> {
        self.get(&format!("/tv/{id}"), &[]).await
    }

    pub async fn get_season(&self, tv_id: u64, season_number: u32) -> sf_core::Result<TmdbSeason> {
        self.get(&format!("/tv/{tv_id}/season/{season_number}"), &[]).await
    }

    // -----------------------------------------------------------------------
    // Images
    // -----------------------------------------------------------------------

    /// Download an image from TMDB. `path` is e.g. "/abc123.jpg", `size` is
    /// e.g. "w500" or "original".
    pub async fn download_image(&self, path: &str, size: &str) -> sf_core::Result<Vec<u8>> {
        self.limiter.until_ready().await;
        let url = format!("{IMAGE_BASE_URL}/{size}{path}");
        let resp = self.http.get(&url).send().await
            .map_err(|e| sf_core::Error::Internal(format!("TMDB image download failed: {e}")))?;
        if !resp.status().is_success() {
            return Err(sf_core::Error::Internal(format!("TMDB image {}: {}", resp.status(), url)));
        }
        let bytes = resp.bytes().await
            .map_err(|e| sf_core::Error::Internal(format!("TMDB image read error: {e}")))?;
        Ok(bytes.to_vec())
    }
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct TmdbSearchResponse {
    results: Vec<TmdbSearchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbSearchResult {
    pub id: u64,
    /// Movie title or TV show name.
    #[serde(alias = "name")]
    pub title: Option<String>,
    #[serde(alias = "first_air_date")]
    pub release_date: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub vote_average: Option<f64>,
    pub media_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbMovie {
    pub id: u64,
    pub title: String,
    pub overview: Option<String>,
    pub release_date: Option<String>,
    pub runtime: Option<i32>,
    pub vote_average: Option<f64>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub imdb_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbTvShow {
    pub id: u64,
    pub name: String,
    pub overview: Option<String>,
    pub first_air_date: Option<String>,
    pub vote_average: Option<f64>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub number_of_seasons: Option<i32>,
    pub seasons: Option<Vec<TmdbSeasonSummary>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbSeasonSummary {
    pub id: u64,
    pub season_number: i32,
    pub name: Option<String>,
    pub episode_count: Option<i32>,
    pub poster_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbSeason {
    pub id: u64,
    pub season_number: i32,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub episodes: Option<Vec<TmdbEpisode>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbEpisode {
    pub id: u64,
    pub episode_number: i32,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub still_path: Option<String>,
    pub runtime: Option<i32>,
    pub vote_average: Option<f64>,
}
