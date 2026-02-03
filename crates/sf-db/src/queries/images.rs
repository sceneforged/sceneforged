//! Image/artwork CRUD operations.

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
}
