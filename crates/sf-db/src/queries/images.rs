//! Image/artwork CRUD operations.

use std::collections::HashMap;

use rusqlite::Connection;
use sf_core::{Error, ImageId, ItemId, Result};

use crate::models::Image;

const COLS: &str = "id, item_id, image_type, path, provider, width, height";

/// Create a new image record.
pub fn create_image(
    conn: &Connection,
    item_id: ItemId,
    image_type: &str,
    path: &str,
    provider: Option<&str>,
    width: Option<i32>,
    height: Option<i32>,
) -> Result<Image> {
    let id = ImageId::new();

    conn.execute(
        "INSERT INTO images (id, item_id, image_type, path, provider, width, height)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        rusqlite::params![
            id.to_string(),
            item_id.to_string(),
            image_type,
            path,
            provider,
            width,
            height,
        ],
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(Image {
        id,
        item_id,
        image_type: image_type.to_string(),
        path: path.to_string(),
        provider: provider.map(String::from),
        width,
        height,
    })
}

/// List images for an item.
pub fn list_images_by_item(conn: &Connection, item_id: ItemId) -> Result<Vec<Image>> {
    let q = format!("SELECT {COLS} FROM images WHERE item_id = ?1");
    let mut stmt = conn.prepare(&q).map_err(|e| Error::database(e.to_string()))?;
    let rows = stmt
        .query_map([item_id.to_string()], Image::from_row)
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(rows)
}

/// Batch-fetch images for multiple items in a single query.
///
/// Returns a HashMap keyed by ItemId, with each value being the list of images
/// for that item. Items with no images will not have an entry in the map.
pub fn batch_get_images(
    conn: &Connection,
    item_ids: &[ItemId],
) -> Result<HashMap<ItemId, Vec<Image>>> {
    if item_ids.is_empty() {
        return Ok(HashMap::new());
    }

    // Build IN clause placeholders: ?1, ?2, ...
    let placeholders: Vec<String> = (0..item_ids.len()).map(|i| format!("?{}", i + 1)).collect();
    let in_clause = placeholders.join(",");

    let sql = format!(
        "SELECT {COLS} FROM images WHERE item_id IN ({in_clause})"
    );

    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    for id in item_ids {
        params.push(Box::new(id.to_string()));
    }
    let params_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql).map_err(|e| Error::database(e.to_string()))?;
    let rows = stmt
        .query_map(params_refs.as_slice(), Image::from_row)
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;

    let mut result: HashMap<ItemId, Vec<Image>> = HashMap::new();
    for image in rows {
        result.entry(image.item_id).or_default().push(image);
    }

    Ok(result)
}

/// Delete an image by ID.
pub fn delete_image(conn: &Connection, id: ImageId) -> Result<bool> {
    let n = conn
        .execute("DELETE FROM images WHERE id = ?1", [id.to_string()])
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_memory_pool;
    use crate::queries::{items, libraries};

    fn setup() -> (r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>, ItemId) {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let lib = libraries::create_library(&conn, "M", "movies", &[], &serde_json::json!({})).unwrap();
        let item = items::create_item(
            &conn, lib.id, "movie", "T", None, None, None, None, None, None, None, None, None,
        )
        .unwrap();
        (conn, item.id)
    }

    #[test]
    fn create_list_delete() {
        let (conn, item_id) = setup();
        let img = create_image(&conn, item_id, "primary", "/poster.jpg", Some("tmdb"), Some(1000), Some(1500)).unwrap();
        let list = list_images_by_item(&conn, item_id).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].image_type, "primary");

        assert!(delete_image(&conn, img.id).unwrap());
        assert!(list_images_by_item(&conn, item_id).unwrap().is_empty());
    }

    #[test]
    fn batch_get_images_multiple_items() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let lib = libraries::create_library(&conn, "M", "movies", &[], &serde_json::json!({})).unwrap();

        let item1 = items::create_item(
            &conn, lib.id, "movie", "Movie A", None, None, None, None, None, None, None, None, None,
        ).unwrap();
        let item2 = items::create_item(
            &conn, lib.id, "movie", "Movie B", None, None, None, None, None, None, None, None, None,
        ).unwrap();
        let item3 = items::create_item(
            &conn, lib.id, "movie", "Movie C", None, None, None, None, None, None, None, None, None,
        ).unwrap();

        // item1 gets 2 images, item2 gets 1, item3 gets none.
        create_image(&conn, item1.id, "primary", "/a_poster.jpg", None, None, None).unwrap();
        create_image(&conn, item1.id, "backdrop", "/a_backdrop.jpg", None, None, None).unwrap();
        create_image(&conn, item2.id, "primary", "/b_poster.jpg", Some("tmdb"), Some(500), Some(750)).unwrap();

        let map = batch_get_images(&conn, &[item1.id, item2.id, item3.id]).unwrap();

        // item1 should have 2 images.
        let imgs1 = map.get(&item1.id).unwrap();
        assert_eq!(imgs1.len(), 2);

        // item2 should have 1 image.
        let imgs2 = map.get(&item2.id).unwrap();
        assert_eq!(imgs2.len(), 1);
        assert_eq!(imgs2[0].image_type, "primary");
        assert_eq!(imgs2[0].provider.as_deref(), Some("tmdb"));

        // item3 should not be in the map (no images).
        assert!(map.get(&item3.id).is_none());
    }

    #[test]
    fn batch_get_images_empty_input() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let map = batch_get_images(&conn, &[]).unwrap();
        assert!(map.is_empty());
    }
}
