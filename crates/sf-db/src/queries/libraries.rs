//! Library CRUD operations.

use chrono::Utc;
use rusqlite::Connection;
use sf_core::{Error, LibraryId, Result};

use crate::models::Library;

/// Create a new library.
pub fn create_library(
    conn: &Connection,
    name: &str,
    media_type: &str,
    paths: &[String],
    config: &serde_json::Value,
) -> Result<Library> {
    let id = LibraryId::new();
    let created_at = Utc::now().to_rfc3339();
    let paths_json = serde_json::to_string(paths).map_err(|e| Error::Internal(e.to_string()))?;
    let config_json =
        serde_json::to_string(config).map_err(|e| Error::Internal(e.to_string()))?;

    conn.execute(
        "INSERT INTO libraries (id, name, media_type, paths, config, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            id.to_string(),
            name,
            media_type,
            paths_json,
            config_json,
            created_at
        ],
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(Library {
        id,
        name: name.to_string(),
        media_type: media_type.to_string(),
        paths: paths.to_vec(),
        config: config.clone(),
        created_at,
    })
}

/// Get a library by ID.
pub fn get_library(conn: &Connection, id: LibraryId) -> Result<Option<Library>> {
    let result = conn.query_row(
        "SELECT id, name, media_type, paths, config, created_at FROM libraries WHERE id = ?1",
        [id.to_string()],
        Library::from_row,
    );
    match result {
        Ok(l) => Ok(Some(l)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// List all libraries ordered by name.
pub fn list_libraries(conn: &Connection) -> Result<Vec<Library>> {
    let mut stmt = conn
        .prepare("SELECT id, name, media_type, paths, config, created_at FROM libraries ORDER BY name")
        .map_err(|e| Error::database(e.to_string()))?;
    let rows = stmt
        .query_map([], Library::from_row)
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(rows)
}

/// Delete a library (cascades to items).
pub fn delete_library(conn: &Connection, id: LibraryId) -> Result<bool> {
    let n = conn
        .execute("DELETE FROM libraries WHERE id = ?1", [id.to_string()])
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

/// Update a library's name, media_type, paths, and config.
pub fn update_library(
    conn: &Connection,
    id: LibraryId,
    name: &str,
    media_type: &str,
    paths: &[String],
    config: &serde_json::Value,
) -> Result<bool> {
    let paths_json = serde_json::to_string(paths).map_err(|e| Error::Internal(e.to_string()))?;
    let config_json =
        serde_json::to_string(config).map_err(|e| Error::Internal(e.to_string()))?;

    let n = conn
        .execute(
            "UPDATE libraries SET name = ?1, media_type = ?2, paths = ?3, config = ?4 WHERE id = ?5",
            rusqlite::params![name, media_type, paths_json, config_json, id.to_string()],
        )
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_memory_pool;

    #[test]
    fn crud() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let lib = create_library(
            &conn,
            "Movies",
            "movies",
            &["/media/movies".into()],
            &serde_json::json!({}),
        )
        .unwrap();
        assert_eq!(lib.name, "Movies");

        let found = get_library(&conn, lib.id).unwrap().unwrap();
        assert_eq!(found.paths, vec!["/media/movies".to_string()]);

        let libs = list_libraries(&conn).unwrap();
        assert_eq!(libs.len(), 1);

        assert!(update_library(
            &conn,
            lib.id,
            "Films",
            "movies",
            &["/new".into()],
            &serde_json::json!({"scan": true}),
        )
        .unwrap());

        let updated = get_library(&conn, lib.id).unwrap().unwrap();
        assert_eq!(updated.name, "Films");

        assert!(delete_library(&conn, lib.id).unwrap());
        assert!(get_library(&conn, lib.id).unwrap().is_none());
    }
}
