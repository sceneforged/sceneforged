//! Background enrichment queue for asynchronously processing metadata lookups.
//!
//! The [`EnrichmentQueue`] accepts [`EnrichmentJob`] submissions and processes
//! them in a spawned background task, rate-limiting requests and broadcasting
//! [`AppEvent::ItemUpdated`] events on success.
//!
//! # Example
//!
//! ```rust,ignore
//! let queue = EnrichmentQueue::new(enrichment_service, pool, event_tx);
//! queue.submit(EnrichmentJob {
//!     item_id,
//!     title: "Interstellar".into(),
//!     year: Some(2014),
//!     media_type: MediaType::Movies,
//! }).await?;
//! ```

use std::sync::Arc;

use anyhow::Result;
use sceneforged_common::{ItemId, MediaType};
use sceneforged_db::pool::DbPool;
use sceneforged_db::queries::items;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

use crate::state::AppEvent;

use super::enrichment::EnrichmentService;

/// Channel capacity for the enrichment job queue.
const QUEUE_CAPACITY: usize = 100;

/// Minimum interval between consecutive enrichment jobs to avoid overwhelming
/// external metadata providers.
const RATE_LIMIT: Duration = Duration::from_millis(250);

/// A request to enrich a single media item with external metadata.
#[derive(Debug, Clone)]
pub struct EnrichmentJob {
    /// The database ID of the item to enrich.
    pub item_id: ItemId,
    /// The title to search for in metadata providers.
    pub title: String,
    /// Optional release year to narrow search results.
    pub year: Option<u16>,
    /// Whether this is a movie, TV show, etc.
    pub media_type: MediaType,
}

/// Handle to a background enrichment processing queue.
///
/// Submit jobs via [`submit`](Self::submit); they are processed sequentially by
/// a spawned Tokio task with rate limiting between requests. The background task
/// runs until all [`EnrichmentQueue`] handles (and their inner senders) are
/// dropped, at which point the channel closes and the task exits gracefully.
pub struct EnrichmentQueue {
    sender: mpsc::Sender<EnrichmentJob>,
}

impl EnrichmentQueue {
    /// Create a new enrichment queue and spawn its background processing task.
    ///
    /// # Arguments
    ///
    /// * `enrichment_service` - Shared enrichment service for performing metadata lookups.
    /// * `pool` - Database connection pool for fetching updated items after enrichment.
    /// * `event_tx` - Broadcast channel sender for emitting `AppEvent::ItemUpdated` events.
    ///
    /// The background task processes jobs one at a time with a 250ms delay between
    /// each to rate-limit external API calls. It runs until the channel is closed
    /// (i.e., all sender handles are dropped).
    pub fn new(
        enrichment_service: Arc<EnrichmentService>,
        pool: DbPool,
        event_tx: broadcast::Sender<AppEvent>,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(QUEUE_CAPACITY);

        tokio::spawn(process_jobs(receiver, enrichment_service, pool, event_tx));

        Self { sender }
    }

    /// Submit an enrichment job to the background queue.
    ///
    /// Returns an error if the background processing task has stopped (channel closed).
    pub async fn submit(&self, job: EnrichmentJob) -> Result<()> {
        info!(
            item_id = %job.item_id,
            title = %job.title,
            year = ?job.year,
            media_type = %job.media_type,
            "Submitting enrichment job to queue"
        );

        self.sender
            .send(job)
            .await
            .map_err(|_| anyhow::anyhow!("Enrichment queue is closed"))?;

        Ok(())
    }
}

/// Background loop that drains the job channel, enriches each item, and
/// broadcasts update events.
async fn process_jobs(
    mut receiver: mpsc::Receiver<EnrichmentJob>,
    enrichment_service: Arc<EnrichmentService>,
    pool: DbPool,
    event_tx: broadcast::Sender<AppEvent>,
) {
    info!("Enrichment queue worker started");

    while let Some(job) = receiver.recv().await {
        let item_id = job.item_id;

        info!(
            item_id = %item_id,
            title = %job.title,
            "Processing enrichment job"
        );

        match enrichment_service
            .enrich_item(item_id, &job.title, job.year, job.media_type)
            .await
        {
            Ok(()) => {
                info!(item_id = %item_id, "Enrichment succeeded");

                // Fetch the updated item from the database to broadcast the full event.
                match pool.get() {
                    Ok(conn) => match items::get_item(&conn, item_id) {
                        Ok(Some(item)) => {
                            let _ = event_tx.send(AppEvent::item_updated(item));
                        }
                        Ok(None) => {
                            warn!(
                                item_id = %item_id,
                                "Item not found in database after enrichment; skipping event"
                            );
                        }
                        Err(e) => {
                            warn!(
                                item_id = %item_id,
                                error = %e,
                                "Failed to fetch item after enrichment; skipping event"
                            );
                        }
                    },
                    Err(e) => {
                        warn!(
                            item_id = %item_id,
                            error = %e,
                            "Failed to get database connection for post-enrichment event"
                        );
                    }
                }
            }
            Err(e) => {
                warn!(
                    item_id = %item_id,
                    error = %e,
                    "Enrichment failed; continuing with next job"
                );
            }
        }

        // Rate-limit: wait before processing the next job.
        sleep(RATE_LIMIT).await;
    }

    info!("Enrichment queue worker stopped (channel closed)");
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::metadata::enrichment::EnrichmentService;
    use crate::metadata::images::{ImageService, ImageStorage};
    use crate::metadata::provider::{MediaImages, MediaMetadata, MetadataProvider, SearchResult};
    use crate::metadata::registry::ProviderRegistry;
    use async_trait::async_trait;
    use sceneforged_common::ItemKind;
    use sceneforged_db::models::{Item, ProviderIds};
    use sceneforged_db::queries::libraries::create_library;
    use std::collections::HashMap;

    /// Stub provider that returns canned search results and metadata.
    struct StubProvider {
        search_results: Vec<SearchResult>,
        metadata: MediaMetadata,
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
            Ok(MediaImages {
                posters: Vec::new(),
                backdrops: Vec::new(),
                logos: Vec::new(),
            })
        }

        async fn get_tv_images(&self, _provider_id: &str) -> anyhow::Result<MediaImages> {
            Ok(MediaImages {
                posters: Vec::new(),
                backdrops: Vec::new(),
                logos: Vec::new(),
            })
        }
    }

    fn create_test_enrichment_service(pool: DbPool) -> Arc<EnrichmentService> {
        let dir = tempfile::tempdir().unwrap();
        let storage = ImageStorage::new(dir.path().to_path_buf());
        let image_service = ImageService::new(storage, pool.clone());

        let mut provider_ids = HashMap::new();
        provider_ids.insert("tmdb".to_string(), "99".to_string());

        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(StubProvider {
            search_results: vec![SearchResult {
                id: "99".to_string(),
                title: "Test".to_string(),
                year: Some(2024),
                overview: Some("Overview".to_string()),
                confidence: 0.9,
                provider_name: "stub".to_string(),
                poster_path: None,
            }],
            metadata: MediaMetadata {
                title: "Test".to_string(),
                original_title: None,
                overview: Some("Enriched overview".to_string()),
                genres: vec!["Drama".to_string()],
                production_year: Some(2024),
                premiere_date: None,
                community_rating: Some(7.0),
                runtime_minutes: Some(90),
                provider_ids,
            },
        }));

        Arc::new(EnrichmentService::new(registry, image_service, pool))
    }

    fn insert_test_item(pool: &DbPool) -> ItemId {
        use chrono::Utc;

        let conn = pool.get().unwrap();
        let library = create_library(&conn, "Queue Test Library", MediaType::Movies, &[]).unwrap();

        let item = Item {
            id: ItemId::new(),
            library_id: library.id,
            parent_id: None,
            item_kind: ItemKind::Movie,
            name: "Queue Test Movie".to_string(),
            sort_name: None,
            original_title: None,
            file_path: Some("/media/queue_test.mkv".to_string()),
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
    async fn submit_and_process_job() {
        let pool = sceneforged_db::pool::init_memory_pool().unwrap();
        let item_id = insert_test_item(&pool);
        let service = create_test_enrichment_service(pool.clone());
        let (event_tx, mut event_rx) = broadcast::channel(16);

        let queue = EnrichmentQueue::new(service, pool.clone(), event_tx);

        queue
            .submit(EnrichmentJob {
                item_id,
                title: "Test".into(),
                year: Some(2024),
                media_type: MediaType::Movies,
            })
            .await
            .unwrap();

        // Wait for the background task to process the job.
        let event = tokio::time::timeout(Duration::from_secs(5), event_rx.recv())
            .await
            .expect("Timed out waiting for event")
            .expect("Channel closed");

        match event {
            AppEvent::ItemUpdated { item, .. } => {
                assert_eq!(item.id, item_id);
                assert_eq!(item.overview, Some("Enriched overview".to_string()));
            }
            other => panic!("Expected ItemUpdated event, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn queue_closes_when_sender_dropped() {
        let pool = sceneforged_db::pool::init_memory_pool().unwrap();
        let service = create_test_enrichment_service(pool.clone());
        let (event_tx, _event_rx) = broadcast::channel(16);

        let queue = EnrichmentQueue::new(service, pool, event_tx);

        // Drop the queue (and its sender).
        drop(queue);

        // The background task should exit gracefully. Give it a moment.
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
