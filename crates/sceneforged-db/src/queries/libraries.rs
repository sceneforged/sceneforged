//! Library database queries.
//!
//! This module provides CRUD operations for media libraries.

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use sceneforged_common::{Error, LibraryId, MediaType, Result};
use uuid::Uuid;

use crate::models::Library;

/// Create a new library.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `name` - Library name
/// * `media_type` - Type of media (movies, tvshows, music)
/// * `paths` - List of filesystem paths to scan
///
/// # Returns
///
/// * `Ok(Library)` - The created library
/// * `Err(Error)` - If a database error occurs
pub fn create_library(
    conn: &Connection,
    name: &str,
    media_type: MediaType,
    paths: &[String],
) -> Result<Library> {
    let id = LibraryId::new();
    let created_at = Utc::now();
    let paths_json = serde_json::to_string(paths).map_err(|e| Error::internal(e.to_string()))?;

    conn.execute(
        "INSERT INTO libraries (id, name, media_type, paths, created_at)
         VALUES (:id, :name, :media_type, :paths, :created_at)",
        rusqlite::named_params! {
            ":id": id.to_string(),
            ":name": name,
            ":media_type": media_type.to_string(),
            ":paths": paths_json,
            ":created_at": created_at.to_rfc3339(),
        },
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(Library {
        id,
        name: name.to_string(),
        media_type,
        paths: paths.to_vec(),
        created_at,
    })
}

/// Get a library by ID.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `id` - Library ID
///
/// # Returns
///
/// * `Ok(Some(Library))` - The library if found
/// * `Ok(None)` - If the library does not exist
/// * `Err(Error)` - If a database error occurs
pub fn get_library(conn: &Connection, id: LibraryId) -> Result<Option<Library>> {
    let result = conn.query_row(
        "SELECT id, name, media_type, paths, created_at
         FROM libraries WHERE id = :id",
        rusqlite::named_params! { ":id": id.to_string() },
        |row| {
            let paths_json: String = row.get(3)?;
            let paths: Vec<String> = serde_json::from_str(&paths_json).unwrap_or_default();

            Ok(Library {
                id: LibraryId::from(Uuid::parse_str(&row.get::<_, String>(0)?).unwrap()),
                name: row.get(1)?,
                media_type: serde_json::from_str(&format!("\"{}\"", row.get::<_, String>(2)?))
                    .unwrap(),
                paths,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        },
    );

    match result {
        Ok(library) => Ok(Some(library)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// List all libraries.
///
/// # Arguments
///
/// * `conn` - Database connection
///
/// # Returns
///
/// * `Ok(Vec<Library>)` - List of all libraries
/// * `Err(Error)` - If a database error occurs
pub fn list_libraries(conn: &Connection) -> Result<Vec<Library>> {
    let mut stmt = conn
        .prepare("SELECT id, name, media_type, paths, created_at FROM libraries ORDER BY name")
        .map_err(|e| Error::database(e.to_string()))?;

    let libraries = stmt
        .query_map([], |row| {
            let paths_json: String = row.get(3)?;
            let paths: Vec<String> = serde_json::from_str(&paths_json).unwrap_or_default();

            Ok(Library {
                id: LibraryId::from(Uuid::parse_str(&row.get::<_, String>(0)?).unwrap()),
                name: row.get(1)?,
                media_type: serde_json::from_str(&format!("\"{}\"", row.get::<_, String>(2)?))
                    .unwrap(),
                paths,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        })
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(libraries)
}

/// Update library paths.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `id` - Library ID
/// * `paths` - New list of filesystem paths
///
/// # Returns
///
/// * `Ok(())` - If the update succeeded
/// * `Err(Error)` - If the library does not exist or a database error occurs
pub fn update_library_paths(conn: &Connection, id: LibraryId, paths: &[String]) -> Result<()> {
    let paths_json = serde_json::to_string(paths).map_err(|e| Error::internal(e.to_string()))?;

    let rows_affected = conn
        .execute(
            "UPDATE libraries SET paths = :paths WHERE id = :id",
            rusqlite::named_params! {
                ":id": id.to_string(),
                ":paths": paths_json,
            },
        )
        .map_err(|e| Error::database(e.to_string()))?;

    if rows_affected == 0 {
        return Err(Error::not_found("library"));
    }

    Ok(())
}

/// Update library name.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `id` - Library ID
/// * `name` - New library name
///
/// # Returns
///
/// * `Ok(())` - If the update succeeded
/// * `Err(Error)` - If the library does not exist or a database error occurs
pub fn update_library_name(conn: &Connection, id: LibraryId, name: &str) -> Result<()> {
    let rows_affected = conn
        .execute(
            "UPDATE libraries SET name = :name WHERE id = :id",
            rusqlite::named_params! {
                ":id": id.to_string(),
                ":name": name,
            },
        )
        .map_err(|e| Error::database(e.to_string()))?;

    if rows_affected == 0 {
        return Err(Error::not_found("library"));
    }

    Ok(())
}

/// Delete a library (cascades to items).
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `id` - Library ID to delete
///
/// # Returns
///
/// * `Ok(true)` - If the library was deleted
/// * `Ok(false)` - If the library did not exist
/// * `Err(Error)` - If a database error occurs
pub fn delete_library(conn: &Connection, id: LibraryId) -> Result<bool> {
    let rows_affected = conn
        .execute(
            "DELETE FROM libraries WHERE id = :id",
            rusqlite::named_params! { ":id": id.to_string() },
        )
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(rows_affected > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_memory_pool;

    #[test]
    fn test_create_library() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let paths = vec!["/media/movies".to_string(), "/media/movies2".to_string()];
        let library = create_library(&conn, "Movies", MediaType::Movies, &paths).unwrap();

        assert_eq!(library.name, "Movies");
        assert_eq!(library.media_type, MediaType::Movies);
        assert_eq!(library.paths, paths);
    }

    #[test]
    fn test_get_library() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let paths = vec!["/media/movies".to_string()];
        let created = create_library(&conn, "Movies", MediaType::Movies, &paths).unwrap();

        let found = get_library(&conn, created.id).unwrap();
        assert!(found.is_some());

        let found = found.unwrap();
        assert_eq!(found.id, created.id);
        assert_eq!(found.name, "Movies");
        assert_eq!(found.media_type, MediaType::Movies);
        assert_eq!(found.paths, paths);
    }

    #[test]
    fn test_get_library_not_found() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let result = get_library(&conn, LibraryId::new()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_list_libraries() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        create_library(&conn, "Movies", MediaType::Movies, &[]).unwrap();
        create_library(&conn, "TV Shows", MediaType::TvShows, &[]).unwrap();
        create_library(&conn, "Music", MediaType::Music, &[]).unwrap();

        let libraries = list_libraries(&conn).unwrap();
        assert_eq!(libraries.len(), 3);

        // Should be sorted by name
        assert_eq!(libraries[0].name, "Movies");
        assert_eq!(libraries[1].name, "Music");
        assert_eq!(libraries[2].name, "TV Shows");
    }

    #[test]
    fn test_update_library_paths() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let library = create_library(
            &conn,
            "Movies",
            MediaType::Movies,
            &["/old/path".to_string()],
        )
        .unwrap();

        let new_paths = vec!["/new/path1".to_string(), "/new/path2".to_string()];
        update_library_paths(&conn, library.id, &new_paths).unwrap();

        let updated = get_library(&conn, library.id).unwrap().unwrap();
        assert_eq!(updated.paths, new_paths);
    }

    #[test]
    fn test_update_library_paths_not_found() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let result = update_library_paths(&conn, LibraryId::new(), &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_library_name() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let library = create_library(&conn, "Movies", MediaType::Movies, &[]).unwrap();
        update_library_name(&conn, library.id, "Films").unwrap();

        let updated = get_library(&conn, library.id).unwrap().unwrap();
        assert_eq!(updated.name, "Films");
    }

    #[test]
    fn test_delete_library() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let library = create_library(&conn, "Movies", MediaType::Movies, &[]).unwrap();
        let deleted = delete_library(&conn, library.id).unwrap();
        assert!(deleted);

        let found = get_library(&conn, library.id).unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_delete_library_not_found() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let deleted = delete_library(&conn, LibraryId::new()).unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_delete_library_cascades() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();

        let library = create_library(&conn, "Movies", MediaType::Movies, &[]).unwrap();

        // Insert an item
        conn.execute(
            "INSERT INTO items (id, library_id, item_kind, name) VALUES (?, ?, ?, ?)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                library.id.to_string(),
                "movie",
                "Test"
            ],
        )
        .unwrap();

        // Delete the library
        delete_library(&conn, library.id).unwrap();

        // Verify items were cascade deleted
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM items WHERE library_id = ?",
                [library.id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }
}
