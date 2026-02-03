//! Enrichment service for populating media items with external metadata.
//!
//! The [`EnrichmentService`] orchestrates the full enrichment workflow: searching
//! for a media item across configured providers, fetching detailed metadata and
//! artwork, updating the database record, and downloading images via the
//! [`ImageService`](super::images::ImageService).

use anyhow::{Context, Result};
use chrono::Utc;
use sceneforged_common::{ImageType, ItemId, MediaType};
use sceneforged_db::pool::DbPool;
use sceneforged_db::queries::items;
use tracing::{info, warn};

use super::images::ImageService;

use super::provider::SearchResult;
use super::registry::ProviderRegistry;

/// Service that enriches library items with metadata and artwork from external providers.
///
/// Combines the [`ProviderRegistry`] for metadata lookups, the [`ImageService`]
/// for downloading and storing artwork, and a database connection pool for
/// persisting enriched item data.
///
/// # Example
///
/// ```rust,ignore
/// let service = EnrichmentService::new(registry, image_service, pool);
/// service.enrich_item(item_id, "Interstellar", Some(2014), MediaType::Movies).await?;
/// ```
pub struct EnrichmentService {
    registry: ProviderRegistry,
    image_service: ImageService,
    pool: DbPool,
}

impl EnrichmentService {
    /// Create a new `EnrichmentService`.
    ///
    /// # Arguments
    ///
    /// * `registry` - Provider registry with configured metadata backends
    /// * `image_service` - Image service for downloading and storing artwork
    /// * `pool` - Database connection pool
    pub fn new(registry: ProviderRegistry, image_service: ImageService, pool: DbPool) -> Self {
        Self {
            registry,
            image_service,
            pool,
        }
    }

    /// Enrich a library item with metadata and artwork from external providers.
    ///
    /// This method performs the following steps:
    ///
    /// 1. Searches for the item using the registry (movie or TV based on `media_type`).
    /// 2. Selects the top result (highest confidence).
    /// 3. Fetches full metadata from the provider that returned the top result.
    /// 4. Updates the item in the database with provider IDs, overview, community
    ///    rating, genres, production year, and premiere date.
    /// 5. Fetches available images from the provider.
    /// 6. Downloads and stores the best poster (Primary image type) and best
    ///    backdrop image via the image service.
    ///
    /// Errors during image download are logged but do not prevent metadata from
    /// being saved. If no search results are found, a warning is logged and the
    /// method returns `Ok(())`.
    ///
    /// # Arguments
    ///
    /// * `item_id` - ID of the item to enrich
    /// * `title` - Title to search for
    /// * `year` - Optional release year to narrow the search
    /// * `media_type` - Whether this is a movie or TV show
    ///
    /// # Errors
    ///
    /// Returns an error if the item cannot be found in the database, if the
    /// metadata provider search or fetch fails, or if the database update fails.
    pub async fn enrich_item(
        &self,
        item_id: ItemId,
        title: &str,
        year: Option<u16>,
        media_type: MediaType,
    ) -> Result<()> {
        info!(
            item_id = %item_id,
            title = title,
            year = ?year,
            media_type = %media_type,
            "Starting metadata enrichment"
        );

        // Step 1: Search for the item across providers.
        let results = self.search(title, year, media_type).await?;

        let top_result = match results.first() {
            Some(r) => r,
            None => {
                warn!(
                    item_id = %item_id,
                    title = title,
                    "No metadata search results found; skipping enrichment"
                );
                return Ok(());
            }
        };

        info!(
            item_id = %item_id,
            provider = %top_result.provider_name,
            provider_id = %top_result.id,
            confidence = top_result.confidence,
            "Selected top search result"
        );

        // Step 2: Look up the provider that returned the result.
        let provider = self
            .registry
            .get(&top_result.provider_name)
            .with_context(|| {
                format!(
                    "Provider '{}' not found in registry",
                    top_result.provider_name
                )
            })?;

        // Step 3: Fetch full metadata.
        let metadata = match media_type {
            MediaType::Movies => provider.get_movie_metadata(&top_result.id).await,
            MediaType::TvShows => provider.get_tv_metadata(&top_result.id).await,
            _ => {
                warn!(
                    item_id = %item_id,
                    media_type = %media_type,
                    "Unsupported media type for enrichment; skipping"
                );
                return Ok(());
            }
        }
        .context("Failed to fetch full metadata from provider")?;

        info!(
            item_id = %item_id,
            title = %metadata.title,
            genres = ?metadata.genres,
            "Fetched full metadata"
        );

        // Step 4: Update the item in the database.
        {
            let conn = self
                .pool
                .get()
                .context("Failed to get database connection")?;

            let mut item = items::get_item(&conn, item_id)
                .context("Failed to query item from database")?
                .with_context(|| format!("Item not found in database: {}", item_id))?;

            // Merge provider IDs from the metadata into the item.
            if let Some(tmdb_id) = metadata.provider_ids.get("tmdb") {
                item.provider_ids.tmdb = Some(tmdb_id.clone());
            }
            if let Some(imdb_id) = metadata.provider_ids.get("imdb") {
                item.provider_ids.imdb = Some(imdb_id.clone());
            }
            if let Some(tvdb_id) = metadata.provider_ids.get("tvdb") {
                item.provider_ids.tvdb = Some(tvdb_id.clone());
            }

            // Update metadata fields.
            item.overview = metadata.overview.clone();
            item.community_rating = metadata.community_rating;
            item.genres = metadata.genres.clone();
            item.production_year = metadata.production_year.map(|y| y as i32);
            item.premiere_date = metadata.premiere_date.clone();
            item.date_modified = Utc::now();

            items::upsert_item(&conn, &item).context("Failed to upsert enriched item")?;
        }

        info!(item_id = %item_id, "Saved enriched metadata to database");

        // Step 5: Fetch images from the provider.
        let images_result = match media_type {
            MediaType::Movies => provider.get_movie_images(&top_result.id).await,
            MediaType::TvShows => provider.get_tv_images(&top_result.id).await,
            _ => return Ok(()),
        };

        let images = match images_result {
            Ok(imgs) => imgs,
            Err(e) => {
                warn!(
                    item_id = %item_id,
                    error = %e,
                    "Failed to fetch images from provider; metadata was saved successfully"
                );
                return Ok(());
            }
        };

        // Step 6: Download and store the best poster and backdrop.
        let provider_name = top_result.provider_name.clone();

        // Best poster (Primary image type) -- highest vote_average.
        if let Some(best_poster) = images.posters.iter().max_by(|a, b| {
            a.vote_average
                .partial_cmp(&b.vote_average)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            info!(
                item_id = %item_id,
                url = %best_poster.url,
                "Downloading poster image"
            );
            match self
                .image_service
                .download_and_store(
                    item_id,
                    &best_poster.url,
                    ImageType::Primary,
                    Some(provider_name.clone()),
                )
                .await
            {
                Ok(image_id) => {
                    info!(
                        item_id = %item_id,
                        image_id = %image_id,
                        "Stored poster image"
                    );
                }
                Err(e) => {
                    warn!(
                        item_id = %item_id,
                        error = %e,
                        "Failed to download/store poster image"
                    );
                }
            }
        }

        // Best backdrop -- highest vote_average.
        if let Some(best_backdrop) = images.backdrops.iter().max_by(|a, b| {
            a.vote_average
                .partial_cmp(&b.vote_average)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            info!(
                item_id = %item_id,
                url = %best_backdrop.url,
                "Downloading backdrop image"
            );
            match self
                .image_service
                .download_and_store(
                    item_id,
                    &best_backdrop.url,
                    ImageType::Backdrop,
                    Some(provider_name.clone()),
                )
                .await
            {
                Ok(image_id) => {
                    info!(
                        item_id = %item_id,
                        image_id = %image_id,
                        "Stored backdrop image"
                    );
                }
                Err(e) => {
                    warn!(
                        item_id = %item_id,
                        error = %e,
                        "Failed to download/store backdrop image"
                    );
                }
            }
        }

        info!(item_id = %item_id, "Enrichment complete");

        Ok(())
    }

    /// Search for a media item across all available providers.
    ///
    /// For movies, uses the registry's aggregated search. For TV shows, queries
    /// the primary provider directly (since the registry currently only exposes
    /// movie search).
    async fn search(
        &self,
        title: &str,
        year: Option<u16>,
        media_type: MediaType,
    ) -> Result<Vec<SearchResult>> {
        match media_type {
            MediaType::Movies => self
                .registry
                .search_movie(title, year)
                .await
                .context("Movie search failed"),
            MediaType::TvShows => {
                let provider = self
                    .registry
                    .primary()
                    .context("No metadata provider available for TV search")?;
                provider.search_tv(title).await.context("TV search failed")
            }
            _ => {
                warn!(media_type = %media_type, "Unsupported media type for search");
                Ok(Vec::new())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::images::{ImageService, ImageStorage};
    use crate::metadata::provider::{ImageInfo, MediaImages, MediaMetadata, MetadataProvider};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Arc;

    /// Stub provider for testing enrichment without real network calls.
    struct StubProvider {
        search_results: Vec<SearchResult>,
        metadata: MediaMetadata,
        images: MediaImages,
    }

    #[async_trait]
    impl MetadataProvider for StubProvider {
        fn name(&self) -> &'static str {
            "stub"
        }

        fn is_available(&self) -> bool {
            true
        }

        async fn search_movie(
            &self,
            _title: &str,
            _year: Option<u16>,
        ) -> anyhow::Result<Vec<SearchResult>> {
            Ok(self.search_results.clone())
        }

        async fn search_tv(&self, _title: &str) -> anyhow::Result<Vec<SearchResult>> {
            Ok(self.search_results.clone())
        }

        async fn get_movie_metadata(&self, _provider_id: &str) -> anyhow::Result<MediaMetadata> {
            Ok(self.metadata.clone())
        }

        async fn get_tv_metadata(&self, _provider_id: &str) -> anyhow::Result<MediaMetadata> {
            Ok(self.metadata.clone())
        }

        async fn get_movie_images(&self, _provider_id: &str) -> anyhow::Result<MediaImages> {
            Ok(self.images.clone())
        }

        async fn get_tv_images(&self, _provider_id: &str) -> anyhow::Result<MediaImages> {
            Ok(self.images.clone())
        }
    }

    fn make_test_provider() -> StubProvider {
        let mut provider_ids = HashMap::new();
        provider_ids.insert("tmdb".to_string(), "12345".to_string());
        provider_ids.insert("imdb".to_string(), "tt1234567".to_string());

        StubProvider {
            search_results: vec![SearchResult {
                id: "12345".to_string(),
                title: "Test Movie".to_string(),
                year: Some(2023),
                overview: Some("A test movie".to_string()),
                confidence: 0.95,
                provider_name: "stub".to_string(),
                poster_path: None,
            }],
            metadata: MediaMetadata {
                title: "Test Movie".to_string(),
                original_title: None,
                overview: Some("A great test movie about testing.".to_string()),
                genres: vec!["Action".to_string(), "Sci-Fi".to_string()],
                production_year: Some(2023),
                premiere_date: Some("2023-06-15".to_string()),
                community_rating: Some(8.5),
                runtime_minutes: Some(120),
                provider_ids,
            },
            images: MediaImages {
                posters: vec![ImageInfo {
                    url: "https://example.com/poster.jpg".to_string(),
                    width: 1000,
                    height: 1500,
                    language: Some("en".to_string()),
                    vote_average: 5.5,
                }],
                backdrops: vec![ImageInfo {
                    url: "https://example.com/backdrop.jpg".to_string(),
                    width: 1920,
                    height: 1080,
                    language: None,
                    vote_average: 6.0,
                }],
                logos: Vec::new(),
            },
        }
    }

    fn create_test_service() -> (EnrichmentService, DbPool, tempfile::TempDir) {
        let pool = sceneforged_db::pool::init_memory_pool().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let storage = ImageStorage::new(dir.path().to_path_buf());
        let image_service = ImageService::new(storage, pool.clone());

        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(make_test_provider()));

        let service = EnrichmentService::new(registry, image_service, pool.clone());
        (service, pool, dir)
    }

    /// Helper to insert a test item into the database.
    fn insert_test_item(pool: &DbPool) -> ItemId {
        use sceneforged_common::ItemKind;
        use sceneforged_db::models::{Item, ProviderIds};
        use sceneforged_db::queries::libraries::create_library;

        let conn = pool.get().unwrap();
        let library =
            create_library(&conn, "Test Library", MediaType::Movies, &[]).unwrap();

        let item = Item {
            id: ItemId::new(),
            library_id: library.id,
            parent_id: None,
            item_kind: ItemKind::Movie,
            name: "Test Movie".to_string(),
            sort_name: None,
            original_title: None,
            file_path: Some("/media/test.mkv".to_string()),
            container: Some("mkv".to_string()),
            video_codec: Some("hevc".to_string()),
            audio_codec: Some("aac".to_string()),
            resolution: Some("1920x1080".to_string()),
            runtime_ticks: None,
            size_bytes: None,
            overview: None,
            tagline: None,
            genres: vec![],
            tags: vec![],
            studios: vec![],
            people: vec![],
            community_rating: None,
            critic_rating: None,
            production_year: None,
            premiere_date: None,
            end_date: None,
            official_rating: None,
            provider_ids: ProviderIds::default(),
            scene_release_name: None,
            scene_group: None,
            index_number: None,
            parent_index_number: None,
            etag: None,
            date_created: Utc::now(),
            date_modified: Utc::now(),
            hdr_type: None,
            dolby_vision_profile: None,
        };

        items::upsert_item(&conn, &item).unwrap();
        item.id
    }

    #[tokio::test]
    async fn enrich_item_updates_metadata() {
        let (service, pool, _dir) = create_test_service();
        let item_id = insert_test_item(&pool);

        // Enrichment will succeed for metadata but image downloads will fail
        // (stub URLs are not real), which is expected -- metadata should still
        // be persisted.
        let result = service
            .enrich_item(item_id, "Test Movie", Some(2023), MediaType::Movies)
            .await;
        assert!(result.is_ok());

        // Verify metadata was updated in the database.
        let conn = pool.get().unwrap();
        let item = items::get_item(&conn, item_id).unwrap().unwrap();

        assert_eq!(
            item.overview,
            Some("A great test movie about testing.".to_string())
        );
        assert_eq!(item.community_rating, Some(8.5));
        assert_eq!(item.genres, vec!["Action", "Sci-Fi"]);
        assert_eq!(item.production_year, Some(2023));
        assert_eq!(item.premiere_date, Some("2023-06-15".to_string()));
        assert_eq!(item.provider_ids.tmdb, Some("12345".to_string()));
        assert_eq!(item.provider_ids.imdb, Some("tt1234567".to_string()));
    }

    #[tokio::test]
    async fn enrich_item_no_results_returns_ok() {
        let pool = sceneforged_db::pool::init_memory_pool().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let storage = ImageStorage::new(dir.path().to_path_buf());
        let image_service = ImageService::new(storage, pool.clone());

        // Registry with a provider that returns no results.
        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(StubProvider {
            search_results: Vec::new(),
            metadata: MediaMetadata {
                title: String::new(),
                original_title: None,
                overview: None,
                genres: Vec::new(),
                production_year: None,
                premiere_date: None,
                community_rating: None,
                runtime_minutes: None,
                provider_ids: HashMap::new(),
            },
            images: MediaImages {
                posters: Vec::new(),
                backdrops: Vec::new(),
                logos: Vec::new(),
            },
        }));

        let service = EnrichmentService::new(registry, image_service, pool.clone());
        let item_id = insert_test_item(&pool);

        let result = service
            .enrich_item(item_id, "Nonexistent Movie", None, MediaType::Movies)
            .await;
        assert!(result.is_ok());

        // Item should remain unchanged.
        let conn = pool.get().unwrap();
        let item = items::get_item(&conn, item_id).unwrap().unwrap();
        assert!(item.overview.is_none());
    }
}
