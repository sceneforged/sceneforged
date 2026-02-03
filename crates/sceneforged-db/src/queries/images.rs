//! Image database queries.
//!
//! This module provides CRUD operations for item images/artwork,
//! including insert, get, upsert, and delete operations.

use rusqlite::Connection;
use sceneforged_common::{Error, ImageId, ImageType, ItemId, Result};
use uuid::Uuid;

use crate::models::Image;

/// Parse an image from a database row.
///
/// Expects columns in order: id, item_id, image_type, path, provider, width, height, tag.
fn parse_image_row(row: &rusqlite::Row) -> rusqlite::Result<Image> {
    Ok(Image {
        id: ImageId::from(Uuid::parse_str(&row.get::<_, String>(0)?).unwrap()),
        item_id: ItemId::from(Uuid::parse_str(&row.get::<_, String>(1)?).unwrap()),
        image_type: serde_json::from_str(&format!("\"{}\"", row.get::<_, String>(2)?)).unwrap(),
        path: row.get(3)?,
        provider: row.get(4)?,
        width: row.get(5)?,
        height: row.get(6)?,
        tag: row.get(7)?,
    })
}

/// Insert a new image record.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `image` - Image to insert
///
/// # Returns
///
/// * `Ok(ImageId)` - The ID of the inserted image
/// * `Err(Error)` - If a database error occurs
pub fn insert_image(conn: &Connection, image: &Image) -> Result<ImageId> {
    conn.execute(
        "INSERT INTO images (id, item_id, image_type, path, provider, width, height, tag)
         VALUES (:id, :item_id, :image_type, :path, :provider, :width, :height, :tag)",
        rusqlite::named_params! {
            ":id": image.id.to_string(),
            ":item_id": image.item_id.to_string(),
            ":image_type": image.image_type.to_string(),
            ":path": &image.path,
            ":provider": &image.provider,
            ":width": image.width,
            ":height": image.height,
            ":tag": &image.tag,
        },
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(image.id)
}

/// Get an image by ID.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `id` - Image ID
///
/// # Returns
///
/// * `Ok(Some(Image))` - The image if found
/// * `Ok(None)` - If the image does not exist
/// * `Err(Error)` - If a database error occurs
pub fn get_image(conn: &Connection, id: ImageId) -> Result<Option<Image>> {
    let result = conn.query_row(
        "SELECT id, item_id, image_type, path, provider, width, height, tag
         FROM images WHERE id = :id",
        rusqlite::named_params! { ":id": id.to_string() },
        parse_image_row,
    );

    match result {
        Ok(image) => Ok(Some(image)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Get all images for an item.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `item_id` - Item ID
///
/// # Returns
///
/// * `Ok(Vec<Image>)` - List of images for the item
/// * `Err(Error)` - If a database error occurs
pub fn get_images_for_item(conn: &Connection, item_id: ItemId) -> Result<Vec<Image>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, item_id, image_type, path, provider, width, height, tag
             FROM images
             WHERE item_id = :item_id",
        )
        .map_err(|e| Error::database(e.to_string()))?;

    let images = stmt
        .query_map(
            rusqlite::named_params! { ":item_id": item_id.to_string() },
            parse_image_row,
        )
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(images)
}

/// Get the primary image for an item.
///
/// Returns the first image with `image_type = 'primary'` for the given item.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `item_id` - Item ID
///
/// # Returns
///
/// * `Ok(Some(Image))` - The primary image if found
/// * `Ok(None)` - If no primary image exists for the item
/// * `Err(Error)` - If a database error occurs
pub fn get_primary_image(conn: &Connection, item_id: ItemId) -> Result<Option<Image>> {
    let result = conn.query_row(
        "SELECT id, item_id, image_type, path, provider, width, height, tag
         FROM images
         WHERE item_id = :item_id AND image_type = :image_type
         LIMIT 1",
        rusqlite::named_params! {
            ":item_id": item_id.to_string(),
            ":image_type": ImageType::Primary.to_string(),
        },
        parse_image_row,
    );

    match result {
        Ok(image) => Ok(Some(image)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Get an image by item ID and image type.
///
/// Returns the first image matching the given item and type.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `item_id` - Item ID
/// * `image_type` - Image type to look for
///
/// # Returns
///
/// * `Ok(Some(Image))` - The image if found
/// * `Ok(None)` - If no image of that type exists for the item
/// * `Err(Error)` - If a database error occurs
pub fn get_image_by_type(
    conn: &Connection,
    item_id: ItemId,
    image_type: ImageType,
) -> Result<Option<Image>> {
    let result = conn.query_row(
        "SELECT id, item_id, image_type, path, provider, width, height, tag
         FROM images
         WHERE item_id = :item_id AND image_type = :image_type
         LIMIT 1",
        rusqlite::named_params! {
            ":item_id": item_id.to_string(),
            ":image_type": image_type.to_string(),
        },
        parse_image_row,
    );

    match result {
        Ok(image) => Ok(Some(image)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Delete an image by ID.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `id` - Image ID to delete
///
/// # Returns
///
/// * `Ok(true)` - If the image was deleted
/// * `Ok(false)` - If the image did not exist
/// * `Err(Error)` - If a database error occurs
pub fn delete_image(conn: &Connection, id: ImageId) -> Result<bool> {
    let rows_affected = conn
        .execute(
            "DELETE FROM images WHERE id = :id",
            rusqlite::named_params! { ":id": id.to_string() },
        )
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(rows_affected > 0)
}

/// Delete all images for an item.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `item_id` - Item ID whose images should be deleted
///
/// # Returns
///
/// * `Ok(u64)` - Number of images deleted
/// * `Err(Error)` - If a database error occurs
pub fn delete_images_for_item(conn: &Connection, item_id: ItemId) -> Result<u64> {
    let rows_affected = conn
        .execute(
            "DELETE FROM images WHERE item_id = :item_id",
            rusqlite::named_params! { ":item_id": item_id.to_string() },
        )
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(rows_affected as u64)
}

/// Insert or update an image based on item_id and image_type.
///
/// If an image with the same `item_id` and `image_type` already exists, it will be updated.
/// Otherwise, a new image is inserted. The image ID from the provided `image` is used for
/// new inserts; on conflict the existing row is updated in place.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `image` - Image to upsert
///
/// # Returns
///
/// * `Ok(ImageId)` - The ID of the inserted or existing image
/// * `Err(Error)` - If a database error occurs
pub fn upsert_image(conn: &Connection, image: &Image) -> Result<ImageId> {
    // Try to find an existing image for this item_id + image_type
    let existing = get_image_by_type(conn, image.item_id, image.image_type)?;

    if let Some(existing_image) = existing {
        // Update the existing row
        conn.execute(
            "UPDATE images SET path = :path, provider = :provider, width = :width,
                    height = :height, tag = :tag
             WHERE id = :id",
            rusqlite::named_params! {
                ":id": existing_image.id.to_string(),
                ":path": &image.path,
                ":provider": &image.provider,
                ":width": image.width,
                ":height": image.height,
                ":tag": &image.tag,
            },
        )
        .map_err(|e| Error::database(e.to_string()))?;

        Ok(existing_image.id)
    } else {
        insert_image(conn, image)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_memory_pool;
    use crate::queries::items::{upsert_item, get_images};
    use crate::queries::libraries::create_library;
    use crate::models::Item;
    use chrono::Utc;
    use sceneforged_common::{ImageType, ItemKind, LibraryId, MediaType};

    fn create_test_library(conn: &Connection) -> LibraryId {
        create_library(conn, "Test Library", MediaType::Movies, &[])
            .unwrap()
            .id
    }

    fn create_test_item(conn: &Connection, library_id: LibraryId, name: &str) -> Item {
        let item = Item {
            id: ItemId::new(),
            library_id,
            parent_id: None,
            item_kind: ItemKind::Movie,
            name: name.to_string(),
            sort_name: None,
            original_title: None,
            file_path: Some(format!("/media/{}.mkv", name)),
            container: Some("mkv".to_string()),
            video_codec: Some("hevc".to_string()),
            audio_codec: Some("aac".to_string()),
            resolution: Some("1920x1080".to_string()),
            runtime_ticks: Some(72000000000),
            size_bytes: Some(1024 * 1024 * 1024),
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
            provider_ids: Default::default(),
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

        upsert_item(conn, &item).unwrap();
        item
    }

    fn create_test_image(item_id: ItemId, image_type: ImageType) -> Image {
        Image {
            id: ImageId::new(),
            item_id,
            image_type,
            path: format!("/images/{}.jpg", image_type),
            provider: Some("tmdb".to_string()),
            width: Some(1000),
            height: Some(1500),
            tag: None,
        }
    }

    #[test]
    fn test_insert_and_get_image() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);
        let item = create_test_item(&conn, library_id, "Test Movie");

        let image = create_test_image(item.id, ImageType::Primary);
        let id = insert_image(&conn, &image).unwrap();

        let found = get_image(&conn, id).unwrap().unwrap();
        assert_eq!(found.id, image.id);
        assert_eq!(found.item_id, item.id);
        assert_eq!(found.image_type, ImageType::Primary);
        assert_eq!(found.path, "/images/primary.jpg");
        assert_eq!(found.provider, Some("tmdb".to_string()));
    }

    #[test]
    fn test_get_image_not_found() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let found = get_image(&conn, ImageId::new()).unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_get_images_for_item() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);
        let item = create_test_item(&conn, library_id, "Test Movie");

        insert_image(&conn, &create_test_image(item.id, ImageType::Primary)).unwrap();
        insert_image(&conn, &create_test_image(item.id, ImageType::Backdrop)).unwrap();
        insert_image(&conn, &create_test_image(item.id, ImageType::Logo)).unwrap();

        let images = get_images_for_item(&conn, item.id).unwrap();
        assert_eq!(images.len(), 3);
    }

    #[test]
    fn test_get_images_for_item_empty() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);
        let item = create_test_item(&conn, library_id, "Test Movie");

        let images = get_images_for_item(&conn, item.id).unwrap();
        assert!(images.is_empty());
    }

    #[test]
    fn test_get_primary_image() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);
        let item = create_test_item(&conn, library_id, "Test Movie");

        insert_image(&conn, &create_test_image(item.id, ImageType::Backdrop)).unwrap();
        insert_image(&conn, &create_test_image(item.id, ImageType::Primary)).unwrap();

        let primary = get_primary_image(&conn, item.id).unwrap().unwrap();
        assert_eq!(primary.image_type, ImageType::Primary);
    }

    #[test]
    fn test_get_primary_image_not_found() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);
        let item = create_test_item(&conn, library_id, "Test Movie");

        insert_image(&conn, &create_test_image(item.id, ImageType::Backdrop)).unwrap();

        let primary = get_primary_image(&conn, item.id).unwrap();
        assert!(primary.is_none());
    }

    #[test]
    fn test_get_image_by_type() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);
        let item = create_test_item(&conn, library_id, "Test Movie");

        insert_image(&conn, &create_test_image(item.id, ImageType::Primary)).unwrap();
        insert_image(&conn, &create_test_image(item.id, ImageType::Logo)).unwrap();

        let logo = get_image_by_type(&conn, item.id, ImageType::Logo).unwrap().unwrap();
        assert_eq!(logo.image_type, ImageType::Logo);

        let banner = get_image_by_type(&conn, item.id, ImageType::Banner).unwrap();
        assert!(banner.is_none());
    }

    #[test]
    fn test_delete_image() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);
        let item = create_test_item(&conn, library_id, "Test Movie");

        let image = create_test_image(item.id, ImageType::Primary);
        let id = insert_image(&conn, &image).unwrap();

        let deleted = delete_image(&conn, id).unwrap();
        assert!(deleted);

        let found = get_image(&conn, id).unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_delete_image_not_found() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let deleted = delete_image(&conn, ImageId::new()).unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_delete_images_for_item() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);
        let item = create_test_item(&conn, library_id, "Test Movie");

        insert_image(&conn, &create_test_image(item.id, ImageType::Primary)).unwrap();
        insert_image(&conn, &create_test_image(item.id, ImageType::Backdrop)).unwrap();
        insert_image(&conn, &create_test_image(item.id, ImageType::Logo)).unwrap();

        let count = delete_images_for_item(&conn, item.id).unwrap();
        assert_eq!(count, 3);

        let images = get_images_for_item(&conn, item.id).unwrap();
        assert!(images.is_empty());
    }

    #[test]
    fn test_delete_images_for_item_empty() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);
        let item = create_test_item(&conn, library_id, "Test Movie");

        let count = delete_images_for_item(&conn, item.id).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_upsert_image_insert() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);
        let item = create_test_item(&conn, library_id, "Test Movie");

        let image = create_test_image(item.id, ImageType::Primary);
        let id = upsert_image(&conn, &image).unwrap();
        assert_eq!(id, image.id);

        let found = get_image(&conn, id).unwrap().unwrap();
        assert_eq!(found.path, "/images/primary.jpg");
    }

    #[test]
    fn test_upsert_image_update() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);
        let item = create_test_item(&conn, library_id, "Test Movie");

        // Insert initial image
        let image1 = create_test_image(item.id, ImageType::Primary);
        let id1 = upsert_image(&conn, &image1).unwrap();

        // Upsert with same item_id + image_type but different data
        let image2 = Image {
            id: ImageId::new(), // Different ID, but should update existing
            item_id: item.id,
            image_type: ImageType::Primary,
            path: "/images/updated_primary.jpg".to_string(),
            provider: Some("fanart".to_string()),
            width: Some(2000),
            height: Some(3000),
            tag: Some("v2".to_string()),
        };

        let id2 = upsert_image(&conn, &image2).unwrap();

        // Should return the original ID since it updated the existing row
        assert_eq!(id2, id1);

        // Verify data was updated
        let found = get_image(&conn, id1).unwrap().unwrap();
        assert_eq!(found.path, "/images/updated_primary.jpg");
        assert_eq!(found.provider, Some("fanart".to_string()));
        assert_eq!(found.width, Some(2000));
        assert_eq!(found.height, Some(3000));
        assert_eq!(found.tag, Some("v2".to_string()));

        // Should still be only one image for the item
        let all = get_images_for_item(&conn, item.id).unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn test_upsert_different_types_creates_separate_images() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);
        let item = create_test_item(&conn, library_id, "Test Movie");

        upsert_image(&conn, &create_test_image(item.id, ImageType::Primary)).unwrap();
        upsert_image(&conn, &create_test_image(item.id, ImageType::Backdrop)).unwrap();

        let images = get_images_for_item(&conn, item.id).unwrap();
        assert_eq!(images.len(), 2);
    }

    #[test]
    fn test_images_compatible_with_items_get_images() {
        // Verify that images inserted via this module are also visible
        // through the existing items::get_images function.
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let library_id = create_test_library(&conn);
        let item = create_test_item(&conn, library_id, "Test Movie");

        insert_image(&conn, &create_test_image(item.id, ImageType::Primary)).unwrap();

        // The existing items::get_images should also see this image
        let images = get_images(&conn, item.id).unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].image_type, ImageType::Primary);
    }
}
