//! Image service coordinating storage and database operations.
//!
//! Provides high-level operations for storing images with automatic
//! database record creation and URL-based image downloading.

use std::path::PathBuf;

use anyhow::{Context, Result};
use sceneforged_common::{ImageId, ImageType, ItemId};
use sceneforged_db::models::Image;
use sceneforged_db::pool::DbPool;
use sceneforged_db::queries::images;

use super::storage::{ImageSize, ImageStorage};

/// High-level image service that coordinates filesystem storage with database records.
pub struct ImageService {
    storage: ImageStorage,
    pool: DbPool,
}

impl ImageService {
    /// Create a new `ImageService`.
    ///
    /// # Arguments
    ///
    /// * `storage` - The filesystem image storage backend
    /// * `pool` - Database connection pool
    pub fn new(storage: ImageStorage, pool: DbPool) -> Self {
        Self { storage, pool }
    }

    /// Store image data to disk and create a database record.
    ///
    /// This writes the image and all size variants to the filesystem, then
    /// inserts (or upserts) a record in the database linking the image to
    /// the given item.
    ///
    /// # Arguments
    ///
    /// * `item_id` - The item this image belongs to
    /// * `data` - Raw image bytes
    /// * `image_type` - The type of image (primary, backdrop, etc.)
    /// * `provider` - Optional provider name (e.g., "tmdb", "fanart")
    ///
    /// # Returns
    ///
    /// The `ImageId` of the newly created database record.
    pub fn store_and_record(
        &self,
        item_id: ItemId,
        data: &[u8],
        image_type: ImageType,
        provider: Option<String>,
    ) -> Result<ImageId> {
        // Store to disk and generate size variants
        let stored = self
            .storage
            .store(&item_id, image_type, data)
            .context("Failed to store image to disk")?;

        // Create database record
        let image = Image {
            id: ImageId::new(),
            item_id,
            image_type,
            path: stored.path,
            provider,
            width: Some(stored.width as i32),
            height: Some(stored.height as i32),
            tag: Some(stored.hash),
        };

        let conn = self
            .pool
            .get()
            .context("Failed to get database connection")?;

        let image_id = images::upsert_image(&conn, &image)
            .context("Failed to upsert image record in database")?;

        Ok(image_id)
    }

    /// Look up a database image record and return the filesystem path for the requested size.
    ///
    /// # Arguments
    ///
    /// * `image_id` - The database ID of the image
    /// * `size` - The desired size variant
    ///
    /// # Returns
    ///
    /// The absolute filesystem path to the requested image variant.
    pub fn get_image_path(&self, image_id: ImageId, size: ImageSize) -> Result<PathBuf> {
        let conn = self
            .pool
            .get()
            .context("Failed to get database connection")?;

        let image = images::get_image(&conn, image_id)
            .context("Failed to query image from database")?
            .ok_or_else(|| anyhow::anyhow!("Image not found: {}", image_id))?;

        let tag = image
            .tag
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Image {} has no content hash tag", image_id))?;

        Ok(self
            .storage
            .get_path(&image.item_id, image.image_type, tag, size))
    }

    /// Download an image from a URL and store it.
    ///
    /// Downloads the image data from the given URL, then delegates to
    /// [`store_and_record`](Self::store_and_record) to persist it.
    ///
    /// # Arguments
    ///
    /// * `item_id` - The item this image belongs to
    /// * `url` - URL to download the image from
    /// * `image_type` - The type of image (primary, backdrop, etc.)
    /// * `provider` - Optional provider name
    ///
    /// # Returns
    ///
    /// The `ImageId` of the newly created database record.
    pub async fn download_and_store(
        &self,
        item_id: ItemId,
        url: &str,
        image_type: ImageType,
        provider: Option<String>,
    ) -> Result<ImageId> {
        let data = reqwest::get(url)
            .await
            .with_context(|| format!("Failed to download image from {}", url))?
            .error_for_status()
            .with_context(|| format!("HTTP error downloading image from {}", url))?
            .bytes()
            .await
            .with_context(|| format!("Failed to read image bytes from {}", url))?;

        self.store_and_record(item_id, &data, image_type, provider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_creation() {
        let dir = tempfile::tempdir().unwrap();
        let storage = ImageStorage::new(dir.path().to_path_buf());
        let pool = sceneforged_db::pool::init_memory_pool().unwrap();
        let _service = ImageService::new(storage, pool);
    }
}
