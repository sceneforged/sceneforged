//! Conversion management logic.
//!
//! This module provides the core conversion management functionality:
//! - Determining viable conversion targets for media items
//! - Queuing conversion jobs
//! - Batch conversion operations

use anyhow::{Context, Result};
use sceneforged_common::{ItemId, Profile};
use sceneforged_db::{pool::DbPool, queries};
use serde::{Deserialize, Serialize};

/// Conversion options for a media item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionOptions {
    /// Profiles that currently exist for this item.
    pub current_profiles: Vec<Profile>,
    /// Profiles that can be created via conversion.
    pub viable_targets: Vec<Profile>,
}

/// Conversion manager for queuing and managing conversion jobs.
pub struct ConversionManager {
    pool: DbPool,
}

impl ConversionManager {
    /// Create a new conversion manager.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Get conversion options for a media item.
    ///
    /// Returns the currently available profiles and what profiles can be created
    /// via conversion based on the source file's characteristics.
    ///
    /// # Arguments
    ///
    /// * `item_id` - The ID of the media item to check
    ///
    /// # Returns
    ///
    /// * `Ok(ConversionOptions)` - Available and viable profile information
    /// * `Err` - If the item or its files cannot be found
    pub fn get_conversion_options(&self, item_id: ItemId) -> Result<ConversionOptions> {
        let conn = self.pool.get()?;

        // Get all media files for this item
        let files = queries::media_files::list_media_files_for_item(&conn, item_id)
            .context("Failed to list media files for item")?;

        if files.is_empty() {
            return Ok(ConversionOptions {
                current_profiles: vec![],
                viable_targets: vec![],
            });
        }

        // Collect profiles that currently exist
        let mut current_profiles: Vec<Profile> = files.iter().map(|f| f.profile).collect();
        current_profiles.sort();
        current_profiles.dedup();

        // Check if we have a file that actually serves as universal
        // (not just classified as Profile B, but actually usable for HLS)
        let has_universal_file = files.iter().any(|f| {
            f.serves_as_universal || f.role == sceneforged_common::FileRole::Universal
        });

        // Determine viable conversion targets
        let mut viable_targets = Vec::new();

        // Check each file to see what conversions are possible
        for file in &files {
            // Check if this file can be converted to Profile A
            if file.can_be_profile_a && !current_profiles.contains(&Profile::A) {
                if !viable_targets.contains(&Profile::A) {
                    viable_targets.push(Profile::A);
                }
            }

            // Check if this file can be converted to Profile B
            // Allow conversion if no file serves as universal, even if a file
            // is classified as Profile B (it may not have faststart/proper keyframes)
            if file.can_be_profile_b && !has_universal_file {
                if !viable_targets.contains(&Profile::B) {
                    viable_targets.push(Profile::B);
                }
            }
        }

        viable_targets.sort();

        Ok(ConversionOptions {
            current_profiles,
            viable_targets,
        })
    }

    /// Start a conversion job to create specified target profiles.
    ///
    /// Validates that the conversion is viable and creates a conversion job
    /// in the database. Only creates jobs for Profile B conversions currently.
    ///
    /// # Arguments
    ///
    /// * `item_id` - The ID of the media item to convert
    /// * `target_profiles` - The profiles to create via conversion
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<String>)` - List of created job IDs
    /// * `Err` - If validation fails or job creation fails
    pub fn start_conversion(
        &self,
        item_id: ItemId,
        target_profiles: Vec<Profile>,
    ) -> Result<Vec<String>> {
        let conn = self.pool.get()?;

        // Get conversion options to validate the request
        let options = self.get_conversion_options(item_id)?;

        let mut job_ids = Vec::new();

        for target_profile in target_profiles {
            // Validate that this conversion is viable
            if !options.viable_targets.contains(&target_profile) {
                anyhow::bail!(
                    "Profile {:?} is not a viable conversion target for item {}. \
                     Current profiles: {:?}, Viable targets: {:?}",
                    target_profile,
                    item_id,
                    options.current_profiles,
                    options.viable_targets
                );
            }

            // For now, only support Profile B conversions
            if target_profile != Profile::B {
                anyhow::bail!(
                    "Only Profile B conversions are currently supported. \
                     Requested: {:?}",
                    target_profile
                );
            }

            // Find a source file that can be converted to this profile
            let files = queries::media_files::list_media_files_for_item(&conn, item_id)?;

            let source_file = files
                .iter()
                .find(|f| match target_profile {
                    Profile::A => f.can_be_profile_a,
                    Profile::B => f.can_be_profile_b,
                    Profile::C => false, // Cannot convert TO Profile C
                })
                .context(format!(
                    "No source file found that can be converted to Profile {:?}",
                    target_profile
                ))?;

            // Cancel any existing active job for this item (stale from previous run)
            if let Some(active_job) = queries::conversion_jobs::get_active_job_for_item(&conn, item_id)? {
                tracing::info!(
                    "Cancelling stale conversion job {} for item {} (was {:?})",
                    active_job.id,
                    item_id,
                    active_job.status
                );
                let _ = queries::conversion_jobs::cancel_job(&conn, &active_job.id);
            }

            // Create the conversion job
            let job = queries::conversion_jobs::create_conversion_job(
                &conn,
                item_id,
                source_file.id,
            )
            .context("Failed to create conversion job")?;

            job_ids.push(job.id);
        }

        Ok(job_ids)
    }

    /// Start a DV Profile 7 to Profile 8 conversion for an item.
    ///
    /// Validates that the item has DV Profile 7 and no active conversion job,
    /// then creates a conversion job for the DV conversion.
    ///
    /// # Arguments
    ///
    /// * `item_id` - The ID of the media item to convert
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The created job ID
    /// * `Err` - If validation fails or job creation fails
    pub fn start_dv_conversion(&self, item_id: ItemId) -> Result<String> {
        let conn = self.pool.get()?;

        // Get the item and verify it has DV Profile 7
        let item = queries::items::get_item(&conn, item_id)?
            .context("Item not found")?;
        if item.dolby_vision_profile.as_deref() != Some("7") {
            anyhow::bail!("Item does not have DV Profile 7");
        }

        // Get source file
        let files = queries::media_files::list_media_files_for_item(&conn, item_id)?;
        let source = files
            .iter()
            .find(|f| f.role == sceneforged_common::FileRole::Source)
            .context("No source file found")?;

        // Cancel any existing active job (stale from previous run)
        if let Some(active_job) = queries::conversion_jobs::get_active_job_for_item(&conn, item_id)? {
            tracing::info!(
                "Cancelling stale conversion job {} for DV conversion of item {}",
                active_job.id,
                item_id
            );
            let _ = queries::conversion_jobs::cancel_job(&conn, &active_job.id);
        }

        // Create conversion job
        let job = queries::conversion_jobs::create_conversion_job(&conn, item_id, source.id)?;

        Ok(job.id)
    }

    /// Batch convert multiple items from DV Profile 7 to Profile 8.
    ///
    /// Attempts to queue a DV conversion for each item. Items that fail validation
    /// (e.g., not DV Profile 7, already have active job) are logged and skipped.
    ///
    /// # Arguments
    ///
    /// * `item_ids` - List of media item IDs to convert
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<String>)` - List of created job IDs (may be fewer than item_ids if some were skipped)
    pub fn batch_dv_convert(&self, item_ids: Vec<ItemId>) -> Result<Vec<String>> {
        let mut job_ids = Vec::new();
        for item_id in item_ids {
            match self.start_dv_conversion(item_id) {
                Ok(id) => job_ids.push(id),
                Err(e) => {
                    tracing::warn!("Failed to queue DV conversion for {}: {}", item_id, e);
                }
            }
        }
        Ok(job_ids)
    }

    /// Queue conversion jobs for multiple items to a single target profile.
    ///
    /// Validates each item individually and only creates jobs for items where
    /// the conversion is viable and not already in progress.
    ///
    /// # Arguments
    ///
    /// * `item_ids` - List of media item IDs to convert
    /// * `target_profile` - The profile to create for all items
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<String>)` - List of created job IDs (may be fewer than item_ids if some were skipped)
    /// * `Err` - If a critical error occurs (note: individual item validation failures are logged but don't fail the batch)
    pub fn batch_convert(
        &self,
        item_ids: Vec<ItemId>,
        target_profile: Profile,
    ) -> Result<Vec<String>> {
        // For now, only support Profile B conversions
        if target_profile != Profile::B {
            anyhow::bail!(
                "Only Profile B conversions are currently supported. \
                 Requested: {:?}",
                target_profile
            );
        }

        let mut job_ids = Vec::new();

        for item_id in item_ids {
            // Try to convert this item, but don't fail the whole batch if one fails
            match self.start_conversion(item_id, vec![target_profile]) {
                Ok(mut ids) => {
                    job_ids.append(&mut ids);
                }
                Err(e) => {
                    // Log the error but continue with other items
                    tracing::warn!(
                        "Failed to queue conversion for item {}: {}",
                        item_id,
                        e
                    );
                }
            }
        }

        Ok(job_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sceneforged_common::{FileRole, LibraryId};
    use sceneforged_db::{pool::init_memory_pool, queries::media_files};

    fn setup_test_db() -> DbPool {
        init_memory_pool().unwrap()
    }

    fn create_test_item(conn: &rusqlite::Connection) -> ItemId {
        let lib_id = LibraryId::new();
        conn.execute(
            "INSERT INTO libraries (id, name, media_type, paths) VALUES (?, ?, ?, ?)",
            rusqlite::params![lib_id.to_string(), "Movies", "movies", "[]"],
        )
        .unwrap();

        let item_id = ItemId::new();
        conn.execute(
            "INSERT INTO items (id, library_id, item_kind, name) VALUES (?, ?, ?, ?)",
            rusqlite::params![
                item_id.to_string(),
                lib_id.to_string(),
                "movie",
                "Test Movie"
            ],
        )
        .unwrap();

        item_id
    }

    #[test]
    fn test_get_conversion_options_no_files() {
        let pool = setup_test_db();
        let manager = ConversionManager::new(pool.clone());

        let conn = pool.get().unwrap();
        let item_id = create_test_item(&conn);

        let options = manager.get_conversion_options(item_id).unwrap();
        assert!(options.current_profiles.is_empty());
        assert!(options.viable_targets.is_empty());
    }

    #[test]
    fn test_get_conversion_options_with_profile_c() {
        let pool = setup_test_db();
        let manager = ConversionManager::new(pool.clone());

        let conn = pool.get().unwrap();
        let item_id = create_test_item(&conn);

        // Create a Profile C file that can be converted to Profile B
        let file = media_files::create_media_file(
            &conn,
            item_id,
            FileRole::Source,
            "/media/movie.mkv",
            1024,
            "mkv",
        )
        .unwrap();

        // Mark it as convertible to Profile B
        media_files::update_media_file_profile(&conn, file.id, Profile::C, false, true).unwrap();

        let options = manager.get_conversion_options(item_id).unwrap();
        assert_eq!(options.current_profiles, vec![Profile::C]);
        assert_eq!(options.viable_targets, vec![Profile::B]);
    }

    #[test]
    fn test_get_conversion_options_with_existing_profile_b() {
        let pool = setup_test_db();
        let manager = ConversionManager::new(pool.clone());

        let conn = pool.get().unwrap();
        let item_id = create_test_item(&conn);

        // Create a Profile B file
        let file = media_files::create_media_file(
            &conn,
            item_id,
            FileRole::Universal,
            "/cache/movie.mp4",
            512,
            "mp4",
        )
        .unwrap();

        media_files::update_media_file_profile(&conn, file.id, Profile::B, false, false).unwrap();

        let options = manager.get_conversion_options(item_id).unwrap();
        assert_eq!(options.current_profiles, vec![Profile::B]);
        // Profile B already exists, so no viable targets
        assert!(options.viable_targets.is_empty());
    }

    #[test]
    fn test_get_conversion_options_hdr_source() {
        let pool = setup_test_db();
        let manager = ConversionManager::new(pool.clone());

        let conn = pool.get().unwrap();
        let item_id = create_test_item(&conn);

        // Create a Profile C file with HDR that can become A and B
        let file = media_files::create_media_file(
            &conn,
            item_id,
            FileRole::Source,
            "/media/hdr_movie.mkv",
            2048,
            "mkv",
        )
        .unwrap();

        media_files::update_media_file_profile(&conn, file.id, Profile::C, true, true).unwrap();

        let options = manager.get_conversion_options(item_id).unwrap();
        assert_eq!(options.current_profiles, vec![Profile::C]);
        assert_eq!(options.viable_targets, vec![Profile::A, Profile::B]);
    }

    #[test]
    fn test_start_conversion_success() {
        let pool = setup_test_db();
        let manager = ConversionManager::new(pool.clone());

        let conn = pool.get().unwrap();
        let item_id = create_test_item(&conn);

        let file = media_files::create_media_file(
            &conn,
            item_id,
            FileRole::Source,
            "/media/movie.mkv",
            1024,
            "mkv",
        )
        .unwrap();

        media_files::update_media_file_profile(&conn, file.id, Profile::C, false, true).unwrap();

        let job_ids = manager
            .start_conversion(item_id, vec![Profile::B])
            .unwrap();

        assert_eq!(job_ids.len(), 1);
    }

    #[test]
    fn test_start_conversion_not_viable() {
        let pool = setup_test_db();
        let manager = ConversionManager::new(pool.clone());

        let conn = pool.get().unwrap();
        let item_id = create_test_item(&conn);

        let file = media_files::create_media_file(
            &conn,
            item_id,
            FileRole::Source,
            "/media/movie.mkv",
            1024,
            "mkv",
        )
        .unwrap();

        // Mark as NOT convertible to B
        media_files::update_media_file_profile(&conn, file.id, Profile::C, false, false).unwrap();

        let result = manager.start_conversion(item_id, vec![Profile::B]);
        assert!(result.is_err());
    }

    #[test]
    fn test_start_conversion_already_exists() {
        let pool = setup_test_db();
        let manager = ConversionManager::new(pool.clone());

        let conn = pool.get().unwrap();
        let item_id = create_test_item(&conn);

        // Create Profile B file (already exists)
        let file = media_files::create_media_file(
            &conn,
            item_id,
            FileRole::Universal,
            "/cache/movie.mp4",
            512,
            "mp4",
        )
        .unwrap();

        media_files::update_media_file_profile(&conn, file.id, Profile::B, false, false).unwrap();

        let result = manager.start_conversion(item_id, vec![Profile::B]);
        assert!(result.is_err());
    }

    #[test]
    fn test_batch_convert() {
        let pool = setup_test_db();
        let manager = ConversionManager::new(pool.clone());

        let conn = pool.get().unwrap();

        // Create 3 items
        let item1 = create_test_item(&conn);
        let item2 = create_test_item(&conn);
        let item3 = create_test_item(&conn);

        // Item 1: Can convert to B
        let file1 = media_files::create_media_file(
            &conn,
            item1,
            FileRole::Source,
            "/media/movie1.mkv",
            1024,
            "mkv",
        )
        .unwrap();
        media_files::update_media_file_profile(&conn, file1.id, Profile::C, false, true).unwrap();

        // Item 2: Can convert to B
        let file2 = media_files::create_media_file(
            &conn,
            item2,
            FileRole::Source,
            "/media/movie2.mkv",
            1024,
            "mkv",
        )
        .unwrap();
        media_files::update_media_file_profile(&conn, file2.id, Profile::C, false, true).unwrap();

        // Item 3: Already has Profile B
        let file3 = media_files::create_media_file(
            &conn,
            item3,
            FileRole::Universal,
            "/cache/movie3.mp4",
            512,
            "mp4",
        )
        .unwrap();
        media_files::update_media_file_profile(&conn, file3.id, Profile::B, false, false).unwrap();

        let job_ids = manager
            .batch_convert(vec![item1, item2, item3], Profile::B)
            .unwrap();

        // Should only create jobs for items 1 and 2 (item 3 already has Profile B)
        assert_eq!(job_ids.len(), 2);
    }

    #[test]
    fn test_batch_convert_profile_a_unsupported() {
        let pool = setup_test_db();
        let manager = ConversionManager::new(pool.clone());

        let conn = pool.get().unwrap();
        let item_id = create_test_item(&conn);

        let result = manager.batch_convert(vec![item_id], Profile::A);
        assert!(result.is_err());
    }
}
