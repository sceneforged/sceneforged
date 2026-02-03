//! Media library scanner.
//!
//! This module provides functionality for scanning directories to discover,
//! identify, and import media files into the library database.

pub mod classifier;
pub mod identifier;
pub mod prober;
pub mod qualifier;

use crate::config::Config;
use crate::metadata::queue::{EnrichmentJob, EnrichmentQueue};
use crate::state::AppEvent;
use anyhow::Result;
use sceneforged_common::{
    paths::is_video_file, FileRole, ItemId, ItemKind, LibraryId, MediaFileId, MediaType, Profile,
};
use sceneforged_db::{
    models::Item,
    pool::DbPool,
    queries::{conversion_jobs, hls_cache, items, libraries, media_files},
};
use std::sync::Arc;
use std::path::{Path, PathBuf};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};
use walkdir::WalkDir;

pub use classifier::ProfileClassifier;
pub use identifier::MediaIdentifier;
pub use prober::FileProber;
pub use qualifier::SourceQualifier;

/// Scanner for discovering and importing media files.
pub struct Scanner {
    pool: DbPool,
    config: Arc<Config>,
    prober: FileProber,
    qualifier: SourceQualifier,
    classifier: ProfileClassifier,
    identifier: MediaIdentifier,
    event_tx: Option<broadcast::Sender<AppEvent>>,
    enrichment_queue: Option<Arc<EnrichmentQueue>>,
}

/// Result of scanning a single file.
#[derive(Debug)]
pub struct ScanResult {
    pub item_id: ItemId,
    pub media_file_id: MediaFileId,
    pub serves_as_universal: bool,
    pub needs_conversion: bool,
    /// The full Item that was created/updated.
    pub item: Item,
}

/// Progress callback for scan operations.
pub type ProgressCallback = Box<dyn Fn(ScanProgress) + Send + Sync>;

/// Scan progress information.
#[derive(Debug, Clone)]
pub struct ScanProgress {
    pub files_found: usize,
    pub files_processed: usize,
    pub files_added: usize,
    pub files_skipped: usize,
    pub current_file: Option<PathBuf>,
}

impl Scanner {
    /// Create a new scanner with database pool and config.
    pub fn new(pool: DbPool, config: Arc<Config>) -> Self {
        Self {
            pool,
            config,
            prober: FileProber::new(),
            qualifier: SourceQualifier::new(),
            classifier: ProfileClassifier::new(),
            identifier: MediaIdentifier::new(),
            event_tx: None,
            enrichment_queue: None,
        }
    }

    /// Create a new scanner with event broadcasting support.
    pub fn with_events(
        pool: DbPool,
        config: Arc<Config>,
        event_tx: broadcast::Sender<AppEvent>,
    ) -> Self {
        Self {
            pool,
            config,
            prober: FileProber::new(),
            qualifier: SourceQualifier::new(),
            classifier: ProfileClassifier::new(),
            identifier: MediaIdentifier::new(),
            event_tx: Some(event_tx),
            enrichment_queue: None,
        }
    }

    /// Set the enrichment queue for background metadata lookups after scanning.
    pub fn with_enrichment_queue(mut self, queue: Arc<EnrichmentQueue>) -> Self {
        self.enrichment_queue = Some(queue);
        self
    }

    /// Broadcast an event if the event sender is configured.
    fn broadcast(&self, event: AppEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event);
        }
    }

    /// Scan a library by ID, discovering and importing all media files.
    pub fn scan_library(&self, library_id: LibraryId) -> Result<Vec<ScanResult>> {
        let conn = self.pool.get()?;
        let library = libraries::get_library(&conn, library_id)?
            .ok_or_else(|| anyhow::anyhow!("Library not found: {}", library_id))?;

        let mut results = Vec::new();
        for path in &library.paths {
            let path = PathBuf::from(path);
            if path.exists() {
                let scan_results = self.scan_directory(&path, library_id, library.media_type)?;
                results.extend(scan_results);
            } else {
                warn!("Library path does not exist: {:?}", path);
            }
        }

        Ok(results)
    }

    /// Scan a directory for media files.
    pub fn scan_directory(
        &self,
        path: &Path,
        library_id: LibraryId,
        media_type: MediaType,
    ) -> Result<Vec<ScanResult>> {
        info!("Scanning directory: {:?}", path);

        // First pass: collect all video file paths
        let video_files: Vec<PathBuf> = WalkDir::new(path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| !e.path().is_dir() && is_video_file(e.path()))
            .map(|e| e.path().to_path_buf())
            .collect();

        let files_found = video_files.len() as u32;
        info!("Found {} video files in {:?}", files_found, path);

        let library_id_str = library_id.to_string();
        let mut results = Vec::new();
        let mut files_processed: u32 = 0;
        let mut files_added: u32 = 0;

        for file_path in &video_files {
            files_processed += 1;

            // Broadcast progress
            self.broadcast(AppEvent::library_scan_progress(
                library_id_str.clone(),
                files_found,
                files_processed,
                files_added,
                file_path.file_name().and_then(|n| n.to_str()).map(|s| s.to_string()),
            ));

            // Check if already in database
            let conn = self.pool.get()?;
            let file_path_str = file_path.to_string_lossy();
            if let Ok(Some(_)) = items::get_item_by_path(&conn, &file_path_str) {
                debug!("File already in database: {:?}", file_path);
                continue;
            }
            drop(conn);

            // Scan and import the file
            match self.scan_file(file_path, library_id, media_type) {
                Ok(result) => {
                    files_added += 1;
                    // Broadcast item_added immediately so UI updates in real-time
                    self.broadcast(AppEvent::item_added(result.item.clone()));
                    if result.serves_as_universal {
                        self.broadcast(AppEvent::playback_available(result.item_id.to_string()));
                    }
                    // Queue metadata enrichment if available
                    if let Some(ref queue) = self.enrichment_queue {
                        let job = EnrichmentJob {
                            item_id: result.item.id,
                            title: result.item.name.clone(),
                            year: result.item.production_year.and_then(|y| u16::try_from(y).ok()),
                            media_type,
                        };
                        let queue = queue.clone();
                        tokio::spawn(async move {
                            if let Err(e) = queue.submit(job).await {
                                warn!("Failed to queue enrichment: {}", e);
                            }
                        });
                    }
                    results.push(result);
                }
                Err(e) => {
                    warn!("Failed to scan file {:?}: {}", file_path, e);
                }
            }
        }

        info!(
            "Scan complete: {} files added from {:?}",
            results.len(),
            path
        );
        Ok(results)
    }

    /// Scan a single file and add it to the database.
    pub fn scan_file(
        &self,
        path: &Path,
        library_id: LibraryId,
        media_type: MediaType,
    ) -> Result<ScanResult> {
        debug!("Scanning file: {:?}", path);

        // Probe the file for technical metadata
        let media_info = self.prober.probe(path)?;

        // Parse the release name for metadata
        let identification = self.identifier.identify_from_filename(path);

        // Determine item kind based on media type and parsed info
        let item_kind = match media_type {
            MediaType::Movies => ItemKind::Movie,
            MediaType::TvShows => {
                // Use parsed info to refine - if we have season/episode, it's an episode
                ItemKind::Episode
            }
            MediaType::Music => ItemKind::Audio,
        };

        // Use parsed title if available, otherwise fall back to filename
        let name = if !identification.title.is_empty() {
            identification.title.clone()
        } else {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string()
        };

        let file_size = std::fs::metadata(path)?.len() as i64;

        // Create the item with parsed metadata
        let item = Item {
            id: ItemId::new(),
            library_id,
            parent_id: None,
            item_kind,
            name: name.clone(),
            sort_name: Some(name),
            original_title: None,
            file_path: Some(path.to_string_lossy().to_string()),
            container: Some(media_info.container.clone()),
            video_codec: media_info.video_tracks.first().map(|v| v.codec.clone()),
            audio_codec: media_info.audio_tracks.first().map(|a| a.codec.clone()),
            resolution: media_info
                .video_tracks
                .first()
                .map(|v| format!("{}x{}", v.width, v.height)),
            runtime_ticks: media_info.duration.map(|d| {
                (d.as_secs_f64() * 10_000_000.0) as i64 // Ticks are 100-nanosecond intervals
            }),
            size_bytes: Some(file_size),
            overview: None,
            tagline: None,
            genres: vec![],
            tags: vec![],
            studios: vec![],
            people: vec![],
            community_rating: None,
            critic_rating: None,
            // Use parsed year from release name
            production_year: identification.year.map(|y| y as i32),
            premiere_date: None,
            end_date: None,
            official_rating: None,
            provider_ids: Default::default(),
            // Store scene release info from parser
            scene_release_name: Some(identification.scene_release_name),
            scene_group: identification.release_group,
            // Store episode/season info from parser
            index_number: identification.episode.map(|e| e as i32),
            parent_index_number: identification.season.map(|s| s as i32),
            etag: None,
            date_created: chrono::Utc::now(),
            date_modified: chrono::Utc::now(),
            hdr_type: media_info
                .video_tracks
                .first()
                .and_then(|v| v.hdr_format.as_ref().map(|h| format!("{:?}", h))),
            dolby_vision_profile: media_info
                .video_tracks
                .first()
                .and_then(|v| v.dolby_vision.as_ref().map(|dv| format!("{}", dv.profile))),
        };

        // Insert item into database
        let conn = self.pool.get()?;
        items::upsert_item(&conn, &item)?;

        // Check if source qualifies as universal (Profile B compatible)
        let qualification = self.qualifier.check(path, &media_info);

        // Classify the media file into a profile
        let mut classification = self.classifier.classify(&media_info);

        // If classifier says Profile B and qualifier agrees, attempt HLS precomputation.
        // Profile B is ONLY assigned when HLS cache is successfully populated.
        let serves_as_universal = if classification.profile == Profile::B && qualification.serves_as_universal {
            match sceneforged_media::precompute_hls(path) {
                Ok(hls) => {
                    // HLS precomputation succeeded - will store cache after media file creation
                    debug!("HLS precomputation succeeded for {:?}", path);
                    Some(hls)
                }
                Err(e) => {
                    warn!(
                        "HLS precomputation failed for {:?}, downgrading to Profile C: {}",
                        path, e
                    );
                    classification.profile = Profile::C;
                    None
                }
            }
        } else {
            if classification.profile == Profile::B && !qualification.serves_as_universal {
                debug!(
                    "Downgrading {:?} from Profile B to C: {}",
                    path,
                    qualification.disqualification_reasons.join(", ")
                );
                classification.profile = Profile::C;
            }
            None
        };

        // Create media file entry
        let media_file = media_files::create_media_file(
            &conn,
            item.id,
            FileRole::Source,
            &path.to_string_lossy(),
            file_size,
            &media_info.container,
        )?;

        // Update media file with profile classification
        media_files::update_media_file_profile(
            &conn,
            media_file.id,
            classification.profile,
            classification.can_be_profile_a,
            classification.can_be_profile_b,
        )?;

        // If HLS precomputation succeeded, store the cache alongside the profile.
        // This ensures the invariant: Profile B <-> HLS cache exists.
        let has_hls_cache = if let Some(hls) = serves_as_universal {
            let segment_map_bytes = bincode::serialize(&hls.segment_map)
                .map_err(|e| anyhow::anyhow!("Failed to serialize segment map: {}", e))?;

            let hls_entry = hls_cache::HlsCacheEntry {
                media_file_id: media_file.id,
                init_segment: hls.init_segment,
                segment_count: hls.segment_map.segments.len() as u32,
                segment_map: segment_map_bytes,
            };
            hls_cache::store(&conn, &hls_entry)?;
            info!(
                "Stored HLS cache for {:?} ({} segments)",
                path,
                hls_entry.segment_count
            );
            true
        } else {
            false
        };

        // Update media file with probe metadata
        let video = media_info.video_tracks.first();
        let audio = media_info.audio_tracks.first();

        media_files::update_media_file_metadata(
            &conn,
            media_file.id,
            video.map(|v| v.codec.as_str()),
            audio.map(|a| a.codec.as_str()),
            video.map(|v| v.width as i32),
            video.map(|v| v.height as i32),
            media_info
                .duration
                .map(|d| (d.as_secs_f64() * 10_000_000.0) as i64),
            None, // bit_rate
            video.and_then(|v| v.hdr_format.as_ref()).is_some(),
            has_hls_cache, // serves_as_universal only true when HLS cache populated
            qualification.has_faststart,
            qualification.keyframe_interval_secs,
        )?;

        // Queue conversion job if enabled and file is Profile C (needs conversion)
        // Profile A (4K/HDR) and Profile B (already universal) are never auto-queued
        let needs_conversion = !has_hls_cache;
        if self.config.conversion.auto_convert_on_scan
            && needs_conversion
            && classification.profile == Profile::C
        {
            // Check if there's already an active job for this item
            if conversion_jobs::get_active_job_for_item(&conn, item.id)?.is_none() {
                let job = conversion_jobs::create_conversion_job(&conn, item.id, media_file.id)?;
                info!(
                    "Queued conversion job {} for item {} (file: {:?})",
                    job.id, item.id, path
                );
            }
        }

        // Auto-queue DV Profile 7 → Profile 8 conversion if enabled in config
        if self.config.conversion.auto_convert_dv_p7_to_p8 {
            let has_dv_profile_7 = media_info.video_tracks.iter().any(|track| {
                track
                    .dolby_vision
                    .as_ref()
                    .is_some_and(|dv| dv.profile == 7)
            });

            if has_dv_profile_7 {
                // Queue DV conversion job if not already queued
                if conversion_jobs::get_active_job_for_item(&conn, item.id)?.is_none() {
                    let job =
                        conversion_jobs::create_conversion_job(&conn, item.id, media_file.id)?;
                    info!(
                        "Queued DV P7→P8 conversion job {} for item {}",
                        job.id, item.id
                    );
                }
            }
        }

        info!(
            "Added file: {:?} (serves_as_universal: {}, needs_conversion: {})",
            path, has_hls_cache, needs_conversion
        );

        Ok(ScanResult {
            item_id: item.id,
            media_file_id: media_file.id,
            serves_as_universal: has_hls_cache,
            needs_conversion,
            item,
        })
    }

    /// Queue conversion jobs for all items that need conversion.
    ///
    /// This can be used to re-queue conversions after a failed run or
    /// when conversion settings have changed.
    pub fn queue_pending_conversions(&self, library_id: LibraryId) -> Result<Vec<String>> {
        use sceneforged_db::queries::items::{ItemFilter, Pagination, SortOptions};

        let conn = self.pool.get()?;
        let mut job_ids = Vec::new();

        // Find all items in the library
        let filter = ItemFilter {
            library_id: Some(library_id),
            parent_id: None,
            item_kinds: None,
            search_term: None,
            is_favorite: None,
            user_id: None,
        };
        let sort = SortOptions::default();
        let pagination = Pagination {
            offset: 0,
            limit: 10000,
        };

        let items_list = items::list_items(&conn, &filter, &sort, &pagination)?;

        for item in items_list {
            // Check if already has active job
            if conversion_jobs::get_active_job_for_item(&conn, item.id)?.is_some() {
                continue;
            }

            // Check if has universal file
            if let Ok(Some(_)) =
                media_files::get_media_file_by_role(&conn, item.id, FileRole::Universal)
            {
                continue; // Already has universal file
            }

            // Get source file
            if let Ok(Some(source)) =
                media_files::get_media_file_by_role(&conn, item.id, FileRole::Source)
            {
                // Check if source serves as universal
                if source.serves_as_universal {
                    continue;
                }

                // Skip Profile A content (4K/HDR) - only convert manually through UI
                if source.profile == Profile::A {
                    continue;
                }

                // Queue conversion
                let job = conversion_jobs::create_conversion_job(&conn, item.id, source.id)?;
                info!("Queued conversion job {} for item {}", job.id, item.id);
                job_ids.push(job.id);
            }
        }

        Ok(job_ids)
    }
}
