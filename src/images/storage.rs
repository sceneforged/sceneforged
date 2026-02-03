//! Filesystem-level image storage with size variant generation.
//!
//! Handles storing images on disk organized by item ID, generating multiple
//! size variants (original, large, medium, small) using content-hash naming.

use std::io::Cursor;
use std::path::PathBuf;

use anyhow::{Context, Result};
use image::imageops::FilterType;
use image::ImageFormat;
use sceneforged_common::{ImageType, ItemId};
use sha2::{Digest, Sha256};

/// Size variant for stored images.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageSize {
    /// Original resolution.
    Original,
    /// Large variant (500px width).
    Large,
    /// Medium variant (300px width).
    Medium,
    /// Small variant (150px width).
    Small,
}

impl ImageSize {
    /// Returns the target width in pixels for this size variant, or `None` for original.
    fn target_width(&self) -> Option<u32> {
        match self {
            Self::Original => None,
            Self::Large => Some(500),
            Self::Medium => Some(300),
            Self::Small => Some(150),
        }
    }

    /// Returns the suffix used in filenames for this size variant.
    fn suffix(&self) -> &'static str {
        match self {
            Self::Original => "",
            Self::Large => "_large",
            Self::Medium => "_medium",
            Self::Small => "_small",
        }
    }

    /// Returns all size variants.
    fn all() -> &'static [ImageSize] {
        &[
            ImageSize::Original,
            ImageSize::Large,
            ImageSize::Medium,
            ImageSize::Small,
        ]
    }
}

/// Metadata about a stored image file.
pub struct StoredImage {
    /// Content hash (first 16 hex chars of SHA-256).
    pub hash: String,
    /// Width of the original image in pixels.
    pub width: u32,
    /// Height of the original image in pixels.
    pub height: u32,
    /// Relative path from base_dir to the original image file.
    pub path: String,
}

/// Filesystem manager for image storage.
///
/// Organizes images under `{base_dir}/{item_id}/` with content-hash naming
/// and automatic size variant generation.
pub struct ImageStorage {
    base_dir: PathBuf,
}

impl ImageStorage {
    /// Create a new `ImageStorage` with the given base directory.
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Store image data and generate size variants.
    ///
    /// The image is stored at `{base_dir}/{item_id}/{type}_{hash}.jpg` with
    /// additional size variants stored as `{type}_{hash}_large.jpg`, etc.
    ///
    /// # Arguments
    ///
    /// * `item_id` - The item this image belongs to
    /// * `image_type` - The type of image (primary, backdrop, etc.)
    /// * `data` - Raw image bytes
    ///
    /// # Returns
    ///
    /// Metadata about the stored original image including its content hash.
    pub fn store(
        &self,
        item_id: &ItemId,
        image_type: ImageType,
        data: &[u8],
    ) -> Result<StoredImage> {
        // Compute content hash (first 16 hex chars of SHA-256)
        let hash = compute_hash(data);

        // Decode the image
        let img = image::load_from_memory(data)
            .context("Failed to decode image data")?;

        let original_width = img.width();
        let original_height = img.height();

        // Ensure the item directory exists
        let item_dir = self.base_dir.join(item_id.to_string());
        std::fs::create_dir_all(&item_dir)
            .with_context(|| format!("Failed to create image directory: {}", item_dir.display()))?;

        // Store all size variants
        for size in ImageSize::all() {
            let filename = format_filename(image_type, &hash, *size);
            let file_path = item_dir.join(&filename);

            match size.target_width() {
                None => {
                    // Original: write as JPEG directly
                    let mut buf = Cursor::new(Vec::new());
                    img.write_to(&mut buf, ImageFormat::Jpeg)
                        .context("Failed to encode original image as JPEG")?;
                    std::fs::write(&file_path, buf.into_inner())
                        .with_context(|| {
                            format!("Failed to write image file: {}", file_path.display())
                        })?;
                }
                Some(target_width) => {
                    // Only resize if the original is wider than the target
                    if original_width > target_width {
                        let resized = img.resize(
                            target_width,
                            u32::MAX,
                            FilterType::Lanczos3,
                        );
                        let mut buf = Cursor::new(Vec::new());
                        resized
                            .write_to(&mut buf, ImageFormat::Jpeg)
                            .context("Failed to encode resized image as JPEG")?;
                        std::fs::write(&file_path, buf.into_inner())
                            .with_context(|| {
                                format!("Failed to write image file: {}", file_path.display())
                            })?;
                    } else {
                        // Image is already smaller than target; use original
                        let mut buf = Cursor::new(Vec::new());
                        img.write_to(&mut buf, ImageFormat::Jpeg)
                            .context("Failed to encode image as JPEG")?;
                        std::fs::write(&file_path, buf.into_inner())
                            .with_context(|| {
                                format!("Failed to write image file: {}", file_path.display())
                            })?;
                    }
                }
            }
        }

        // Build relative path for the original
        let relative_path = format!(
            "{}/{}",
            item_id,
            format_filename(image_type, &hash, ImageSize::Original),
        );

        Ok(StoredImage {
            hash,
            width: original_width,
            height: original_height,
            path: relative_path,
        })
    }

    /// Get the filesystem path for a specific image size variant.
    ///
    /// # Arguments
    ///
    /// * `item_id` - The item the image belongs to
    /// * `image_type` - The type of image
    /// * `hash` - The content hash of the image
    /// * `size` - The desired size variant
    pub fn get_path(
        &self,
        item_id: &ItemId,
        image_type: ImageType,
        hash: &str,
        size: ImageSize,
    ) -> PathBuf {
        let filename = format_filename(image_type, hash, size);
        self.base_dir.join(item_id.to_string()).join(filename)
    }

    /// Delete an image and all its size variants from disk.
    ///
    /// # Arguments
    ///
    /// * `item_id` - The item the image belongs to
    /// * `image_type` - The type of image
    /// * `hash` - The content hash of the image
    pub fn delete(
        &self,
        item_id: &ItemId,
        image_type: ImageType,
        hash: &str,
    ) -> Result<()> {
        for size in ImageSize::all() {
            let path = self.get_path(item_id, image_type, hash, *size);
            if path.exists() {
                std::fs::remove_file(&path)
                    .with_context(|| {
                        format!("Failed to delete image file: {}", path.display())
                    })?;
            }
        }
        Ok(())
    }
}

/// Compute the content hash for image data.
///
/// Returns the first 16 hex characters of the SHA-256 digest.
fn compute_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    hex::encode(&digest[..8]) // 8 bytes = 16 hex chars
}

/// Format the filename for an image variant.
fn format_filename(image_type: ImageType, hash: &str, size: ImageSize) -> String {
    format!("{}_{}{}.jpg", image_type, hash, size.suffix())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash_length() {
        let hash = compute_hash(b"test data");
        assert_eq!(hash.len(), 16);
    }

    #[test]
    fn test_compute_hash_deterministic() {
        let h1 = compute_hash(b"same data");
        let h2 = compute_hash(b"same data");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_compute_hash_different_data() {
        let h1 = compute_hash(b"data1");
        let h2 = compute_hash(b"data2");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_format_filename_original() {
        let name = format_filename(ImageType::Primary, "abc123", ImageSize::Original);
        assert_eq!(name, "primary_abc123.jpg");
    }

    #[test]
    fn test_format_filename_large() {
        let name = format_filename(ImageType::Backdrop, "def456", ImageSize::Large);
        assert_eq!(name, "backdrop_def456_large.jpg");
    }

    #[test]
    fn test_format_filename_medium() {
        let name = format_filename(ImageType::Logo, "ghi789", ImageSize::Medium);
        assert_eq!(name, "logo_ghi789_medium.jpg");
    }

    #[test]
    fn test_format_filename_small() {
        let name = format_filename(ImageType::Thumb, "jkl012", ImageSize::Small);
        assert_eq!(name, "thumb_jkl012_small.jpg");
    }

    #[test]
    fn test_get_path() {
        let storage = ImageStorage::new(PathBuf::from("/data/images"));
        let item_id = ItemId::new();
        let path = storage.get_path(&item_id, ImageType::Primary, "abc123", ImageSize::Original);
        let expected = PathBuf::from(format!("/data/images/{}/primary_abc123.jpg", item_id));
        assert_eq!(path, expected);
    }

    #[test]
    fn test_get_path_with_size_variant() {
        let storage = ImageStorage::new(PathBuf::from("/data/images"));
        let item_id = ItemId::new();
        let path = storage.get_path(&item_id, ImageType::Primary, "abc123", ImageSize::Small);
        let expected =
            PathBuf::from(format!("/data/images/{}/primary_abc123_small.jpg", item_id));
        assert_eq!(path, expected);
    }

    #[test]
    fn test_image_size_target_widths() {
        assert_eq!(ImageSize::Original.target_width(), None);
        assert_eq!(ImageSize::Large.target_width(), Some(500));
        assert_eq!(ImageSize::Medium.target_width(), Some(300));
        assert_eq!(ImageSize::Small.target_width(), Some(150));
    }

    #[test]
    fn test_store_and_delete() {
        let dir = tempfile::tempdir().unwrap();
        let storage = ImageStorage::new(dir.path().to_path_buf());
        let item_id = ItemId::new();

        // Create a minimal 2x2 JPEG image in memory
        let mut img = image::RgbImage::new(2, 2);
        for pixel in img.pixels_mut() {
            *pixel = image::Rgb([255, 0, 0]);
        }
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, ImageFormat::Jpeg)
            .unwrap();
        let data = buf.into_inner();

        let stored = storage.store(&item_id, ImageType::Primary, &data).unwrap();
        assert_eq!(stored.hash.len(), 16);
        assert_eq!(stored.width, 2);
        assert_eq!(stored.height, 2);

        // Verify all size variants exist
        for size in ImageSize::all() {
            let path = storage.get_path(&item_id, ImageType::Primary, &stored.hash, *size);
            assert!(path.exists(), "Missing variant: {:?}", size);
        }

        // Delete and verify removal
        storage
            .delete(&item_id, ImageType::Primary, &stored.hash)
            .unwrap();
        for size in ImageSize::all() {
            let path = storage.get_path(&item_id, ImageType::Primary, &stored.hash, *size);
            assert!(!path.exists(), "Variant not deleted: {:?}", size);
        }
    }
}
