//! Media library scanner.
//!
//! This module provides functionality for scanning directories to discover,
//! identify, and import media files into the library database.

pub mod classifier;
pub mod identifier;
pub mod prober;
pub mod qualifier;

use anyhow::Result;
use sceneforged_common::{
    paths::is_video_file, FileRole, ItemId, ItemKind, LibraryId, MediaFileId, MediaType,
};
use sceneforged_db::{
    models::Item,
    pool::DbPool,
    queries::{conversion_jobs, items, libraries, media_files},
};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use walkdir::WalkDir;

pub use classifier::ProfileClassifier;
pub use identifier::MediaIdentifier;
pub use prober::FileProber;
pub use qualifier::SourceQualifier;

/// Scanner for discovering and importing media files.
pub struct Scanner {
    pool: DbPool,
    prober: FileProber,
    qualifier: SourceQualifier,
    classifier: ProfileClassifier,
    identifier: MediaIdentifier,
}

/// Result of scanning a single file.
#[derive(Debug)]
pub struct ScanResult {
    pub item_id: ItemId,
    pub media_file_id: MediaFileId,
    pub serves_as_universal: bool,
    pub needs_conversion: bool,
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
    /// Create a new scanner with database pool.
    pub fn new(pool: DbPool) -> Self {
        Self {
            pool,
            prober: FileProber::new(),
            qualifier: SourceQualifier::new(),
            classifier: ProfileClassifier::new(),
            identifier: MediaIdentifier::new(),
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
        let mut results = Vec::new();

        for entry in WalkDir::new(path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let file_path = entry.path();

            // Skip directories
            if file_path.is_dir() {
                continue;
            }

            // Check if it's a media file
            if !is_video_file(file_path) {
                continue;
            }

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
                Ok(result) => results.push(result),
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
                if identification.season.is_some() || identification.episode.is_some() {
                    ItemKind::Episode
                } else {
                    ItemKind::Episode // Default to episode for TV library
                }
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
        let qualification = self.qualifier.check(&media_info);

        // Classify the media file into a profile
        let classification = self.classifier.classify(&media_info);

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
            qualification.serves_as_universal,
            qualification.has_faststart,
            qualification.keyframe_interval_secs,
        )?;

        // Queue conversion job if file doesn't qualify as universal
        let needs_conversion = !qualification.serves_as_universal;
        if needs_conversion {
            // Check if there's already an active job for this item
            if conversion_jobs::get_active_job_for_item(&conn, item.id)?.is_none() {
                let job = conversion_jobs::create_conversion_job(&conn, item.id, media_file.id)?;
                info!(
                    "Queued conversion job {} for item {} (file: {:?})",
                    job.id, item.id, path
                );
            }
        }

        info!(
            "Added file: {:?} (serves_as_universal: {}, needs_conversion: {})",
            path, qualification.serves_as_universal, needs_conversion
        );

        Ok(ScanResult {
            item_id: item.id,
            media_file_id: media_file.id,
            serves_as_universal: qualification.serves_as_universal,
            needs_conversion,
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

                // Queue conversion
                let job = conversion_jobs::create_conversion_job(&conn, item.id, source.id)?;
                info!("Queued conversion job {} for item {}", job.id, item.id);
                job_ids.push(job.id);
            }
        }

        Ok(job_ids)
    }
}
