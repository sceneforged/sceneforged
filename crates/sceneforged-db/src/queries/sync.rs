//! Sync database queries for InfuseSync delta sync.
//!
//! This module provides operations for tracking changes to items and user data,
//! managing sync checkpoints per device, and retrieving delta updates.

use chrono::{DateTime, Duration, Utc};
use rusqlite::Connection;
use sceneforged_common::{CheckpointId, Error, ItemId, Result, UserId};
use uuid::Uuid;

use crate::models::SyncCheckpoint;

/// Record an item change in the sync log.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `item_id` - Item ID that changed
/// * `change_type` - Type of change: "added", "updated", or "removed"
///
/// # Returns
///
/// * `Ok(())` - If the log entry was created
/// * `Err(Error)` - If a database error occurs
pub fn log_item_change(conn: &Connection, item_id: ItemId, change_type: &str) -> Result<()> {
    let now = Utc::now();

    conn.execute(
        "INSERT INTO sync_change_log (item_id, change_type, changed_at)
         VALUES (:item_id, :change_type, :changed_at)",
        rusqlite::named_params! {
            ":item_id": item_id.to_string(),
            ":change_type": change_type,
            ":changed_at": now.to_rfc3339(),
        },
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(())
}

/// Record a user data change in the sync log.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `user_id` - User ID
/// * `item_id` - Item ID whose user data changed
///
/// # Returns
///
/// * `Ok(())` - If the log entry was created
/// * `Err(Error)` - If a database error occurs
pub fn log_user_data_change(conn: &Connection, user_id: UserId, item_id: ItemId) -> Result<()> {
    let now = Utc::now();

    conn.execute(
        "INSERT INTO sync_user_data_log (user_id, item_id, changed_at)
         VALUES (:user_id, :item_id, :changed_at)",
        rusqlite::named_params! {
            ":user_id": user_id.to_string(),
            ":item_id": item_id.to_string(),
            ":changed_at": now.to_rfc3339(),
        },
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(())
}

/// Create or update a sync checkpoint for a device.
///
/// If a checkpoint already exists for this user/device combination, it will be updated
/// with the current timestamp. Otherwise, a new checkpoint is created.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `user_id` - User ID
/// * `device_id` - Device ID
///
/// # Returns
///
/// * `Ok(SyncCheckpoint)` - The checkpoint
/// * `Err(Error)` - If a database error occurs
pub fn upsert_checkpoint(
    conn: &Connection,
    user_id: UserId,
    device_id: &str,
) -> Result<SyncCheckpoint> {
    let now = Utc::now();

    // Try to get existing checkpoint
    let existing = conn.query_row(
        "SELECT id, user_id, device_id, item_checkpoint, user_data_checkpoint, created_at, last_sync
         FROM sync_checkpoints
         WHERE user_id = :user_id AND device_id = :device_id",
        rusqlite::named_params! {
            ":user_id": user_id.to_string(),
            ":device_id": device_id,
        },
        |row| {
            Ok(SyncCheckpoint {
                id: CheckpointId::from(Uuid::parse_str(&row.get::<_, String>(0)?).unwrap()),
                user_id: UserId::from(Uuid::parse_str(&row.get::<_, String>(1)?).unwrap()),
                device_id: row.get(2)?,
                item_checkpoint: row.get(3)?,
                user_data_checkpoint: row.get(4)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                    .unwrap()
                    .with_timezone(&Utc),
                last_sync: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        },
    );

    match existing {
        Ok(mut checkpoint) => {
            // Update last_sync timestamp
            conn.execute(
                "UPDATE sync_checkpoints
                 SET last_sync = :last_sync
                 WHERE id = :id",
                rusqlite::named_params! {
                    ":id": checkpoint.id.to_string(),
                    ":last_sync": now.to_rfc3339(),
                },
            )
            .map_err(|e| Error::database(e.to_string()))?;

            checkpoint.last_sync = now;
            Ok(checkpoint)
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            // Create new checkpoint
            let id = CheckpointId::new();

            conn.execute(
                "INSERT INTO sync_checkpoints (id, user_id, device_id, item_checkpoint, user_data_checkpoint, created_at, last_sync)
                 VALUES (:id, :user_id, :device_id, 0, 0, :created_at, :last_sync)",
                rusqlite::named_params! {
                    ":id": id.to_string(),
                    ":user_id": user_id.to_string(),
                    ":device_id": device_id,
                    ":created_at": now.to_rfc3339(),
                    ":last_sync": now.to_rfc3339(),
                },
            )
            .map_err(|e| Error::database(e.to_string()))?;

            Ok(SyncCheckpoint {
                id,
                user_id,
                device_id: device_id.to_string(),
                item_checkpoint: 0,
                user_data_checkpoint: 0,
                created_at: now,
                last_sync: now,
            })
        }
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Get a checkpoint by ID.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `id` - Checkpoint ID
///
/// # Returns
///
/// * `Ok(Some(SyncCheckpoint))` - The checkpoint if found
/// * `Ok(None)` - If the checkpoint does not exist
/// * `Err(Error)` - If a database error occurs
pub fn get_checkpoint(conn: &Connection, id: CheckpointId) -> Result<Option<SyncCheckpoint>> {
    let result = conn.query_row(
        "SELECT id, user_id, device_id, item_checkpoint, user_data_checkpoint, created_at, last_sync
         FROM sync_checkpoints
         WHERE id = :id",
        rusqlite::named_params! { ":id": id.to_string() },
        |row| {
            Ok(SyncCheckpoint {
                id: CheckpointId::from(Uuid::parse_str(&row.get::<_, String>(0)?).unwrap()),
                user_id: UserId::from(Uuid::parse_str(&row.get::<_, String>(1)?).unwrap()),
                device_id: row.get(2)?,
                item_checkpoint: row.get(3)?,
                user_data_checkpoint: row.get(4)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                    .unwrap()
                    .with_timezone(&Utc),
                last_sync: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        },
    );

    match result {
        Ok(checkpoint) => Ok(Some(checkpoint)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Get checkpoint by user and device.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `user_id` - User ID
/// * `device_id` - Device ID
///
/// # Returns
///
/// * `Ok(Some(SyncCheckpoint))` - The checkpoint if found
/// * `Ok(None)` - If the checkpoint does not exist
/// * `Err(Error)` - If a database error occurs
pub fn get_checkpoint_by_device(
    conn: &Connection,
    user_id: UserId,
    device_id: &str,
) -> Result<Option<SyncCheckpoint>> {
    let result = conn.query_row(
        "SELECT id, user_id, device_id, item_checkpoint, user_data_checkpoint, created_at, last_sync
         FROM sync_checkpoints
         WHERE user_id = :user_id AND device_id = :device_id",
        rusqlite::named_params! {
            ":user_id": user_id.to_string(),
            ":device_id": device_id,
        },
        |row| {
            Ok(SyncCheckpoint {
                id: CheckpointId::from(Uuid::parse_str(&row.get::<_, String>(0)?).unwrap()),
                user_id: UserId::from(Uuid::parse_str(&row.get::<_, String>(1)?).unwrap()),
                device_id: row.get(2)?,
                item_checkpoint: row.get(3)?,
                user_data_checkpoint: row.get(4)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                    .unwrap()
                    .with_timezone(&Utc),
                last_sync: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        },
    );

    match result {
        Ok(checkpoint) => Ok(Some(checkpoint)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Get items changed since a checkpoint.
///
/// Returns tuples of (log_id, item_id, change_type).
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `since_id` - Log ID to query changes since (exclusive)
///
/// # Returns
///
/// * `Ok(Vec<(i64, ItemId, String)>)` - List of changes
/// * `Err(Error)` - If a database error occurs
pub fn get_changed_items(conn: &Connection, since_id: i64) -> Result<Vec<(i64, ItemId, String)>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, item_id, change_type
             FROM sync_change_log
             WHERE id > :since_id
             ORDER BY id ASC",
        )
        .map_err(|e| Error::database(e.to_string()))?;

    let changes = stmt
        .query_map(rusqlite::named_params! { ":since_id": since_id }, |row| {
            Ok((
                row.get(0)?,
                ItemId::from(Uuid::parse_str(&row.get::<_, String>(1)?).unwrap()),
                row.get(2)?,
            ))
        })
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(changes)
}

/// Get removed item IDs since a checkpoint.
///
/// Returns tuples of (log_id, item_id) for items that were removed.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `since_id` - Log ID to query changes since (exclusive)
///
/// # Returns
///
/// * `Ok(Vec<(i64, ItemId)>)` - List of removed items
/// * `Err(Error)` - If a database error occurs
pub fn get_removed_items(conn: &Connection, since_id: i64) -> Result<Vec<(i64, ItemId)>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, item_id
             FROM sync_change_log
             WHERE id > :since_id AND change_type = 'removed'
             ORDER BY id ASC",
        )
        .map_err(|e| Error::database(e.to_string()))?;

    let removed = stmt
        .query_map(rusqlite::named_params! { ":since_id": since_id }, |row| {
            Ok((
                row.get(0)?,
                ItemId::from(Uuid::parse_str(&row.get::<_, String>(1)?).unwrap()),
            ))
        })
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(removed)
}

/// Get user data changes since a checkpoint.
///
/// Returns tuples of (log_id, item_id) for items whose user data changed.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `user_id` - User ID
/// * `since_id` - Log ID to query changes since (exclusive)
///
/// # Returns
///
/// * `Ok(Vec<(i64, ItemId)>)` - List of user data changes
/// * `Err(Error)` - If a database error occurs
pub fn get_user_data_changes(
    conn: &Connection,
    user_id: UserId,
    since_id: i64,
) -> Result<Vec<(i64, ItemId)>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, item_id
             FROM sync_user_data_log
             WHERE user_id = :user_id AND id > :since_id
             ORDER BY id ASC",
        )
        .map_err(|e| Error::database(e.to_string()))?;

    let changes = stmt
        .query_map(
            rusqlite::named_params! {
                ":user_id": user_id.to_string(),
                ":since_id": since_id,
            },
            |row| {
                Ok((
                    row.get(0)?,
                    ItemId::from(Uuid::parse_str(&row.get::<_, String>(1)?).unwrap()),
                ))
            },
        )
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(changes)
}

/// Update checkpoint positions.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `id` - Checkpoint ID
/// * `item_checkpoint` - New item checkpoint position
/// * `user_data_checkpoint` - New user data checkpoint position
///
/// # Returns
///
/// * `Ok(())` - If the update succeeded
/// * `Err(Error)` - If the checkpoint does not exist or a database error occurs
pub fn update_checkpoint(
    conn: &Connection,
    id: CheckpointId,
    item_checkpoint: i64,
    user_data_checkpoint: i64,
) -> Result<()> {
    let now = Utc::now();

    let rows_affected = conn
        .execute(
            "UPDATE sync_checkpoints
             SET item_checkpoint = :item_checkpoint,
                 user_data_checkpoint = :user_data_checkpoint,
                 last_sync = :last_sync
             WHERE id = :id",
            rusqlite::named_params! {
                ":id": id.to_string(),
                ":item_checkpoint": item_checkpoint,
                ":user_data_checkpoint": user_data_checkpoint,
                ":last_sync": now.to_rfc3339(),
            },
        )
        .map_err(|e| Error::database(e.to_string()))?;

    if rows_affected == 0 {
        return Err(Error::not_found("checkpoint"));
    }

    Ok(())
}

/// Delete a checkpoint.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `id` - Checkpoint ID to delete
///
/// # Returns
///
/// * `Ok(true)` - If the checkpoint was deleted
/// * `Ok(false)` - If the checkpoint did not exist
/// * `Err(Error)` - If a database error occurs
pub fn delete_checkpoint(conn: &Connection, id: CheckpointId) -> Result<bool> {
    let rows_affected = conn
        .execute(
            "DELETE FROM sync_checkpoints WHERE id = :id",
            rusqlite::named_params! { ":id": id.to_string() },
        )
        .map_err(|e| Error::database(e.to_string()))?;

    Ok(rows_affected > 0)
}

/// Prune old sync logs.
///
/// Deletes sync log entries older than the specified number of days.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `days` - Number of days to keep (entries older than this will be deleted)
///
/// # Returns
///
/// * `Ok(u32)` - Number of entries deleted
/// * `Err(Error)` - If a database error occurs
pub fn prune_sync_logs(conn: &Connection, days: u32) -> Result<u32> {
    let cutoff = Utc::now() - Duration::days(days as i64);

    let item_logs_deleted = conn
        .execute(
            "DELETE FROM sync_change_log WHERE changed_at < :cutoff",
            rusqlite::named_params! { ":cutoff": cutoff.to_rfc3339() },
        )
        .map_err(|e| Error::database(e.to_string()))?;

    let user_data_logs_deleted = conn
        .execute(
            "DELETE FROM sync_user_data_log WHERE changed_at < :cutoff",
            rusqlite::named_params! { ":cutoff": cutoff.to_rfc3339() },
        )
        .map_err(|e| Error::database(e.to_string()))?;

    Ok((item_logs_deleted + user_data_logs_deleted) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_memory_pool;
    use crate::queries::{items::upsert_item, libraries::create_library, users::create_user};
    use sceneforged_common::{ItemKind, MediaType};

    fn create_test_setup(conn: &Connection) -> (UserId, ItemId) {
        let user = create_user(conn, "testuser", "hash", false).unwrap();
        let library = create_library(conn, "Test", MediaType::Movies, &[]).unwrap();

        let item = crate::models::Item {
            id: ItemId::new(),
            library_id: library.id,
            parent_id: None,
            item_kind: ItemKind::Movie,
            name: "Test Movie".to_string(),
            sort_name: None,
            original_title: None,
            file_path: Some("/test.mkv".to_string()),
            container: None,
            video_codec: None,
            audio_codec: None,
            resolution: None,
            runtime_ticks: Some(100000000),
            size_bytes: None,
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
            provider_ids: crate::models::ProviderIds::default(),
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
        (user.id, item.id)
    }

    #[test]
    fn test_log_item_change() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let (_, item_id) = create_test_setup(&conn);

        log_item_change(&conn, item_id, "added").unwrap();

        let changes = get_changed_items(&conn, 0).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].1, item_id);
        assert_eq!(changes[0].2, "added");
    }

    #[test]
    fn test_log_user_data_change() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let (user_id, item_id) = create_test_setup(&conn);

        log_user_data_change(&conn, user_id, item_id).unwrap();

        let changes = get_user_data_changes(&conn, user_id, 0).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].1, item_id);
    }

    #[test]
    fn test_upsert_checkpoint() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let (user_id, _) = create_test_setup(&conn);

        let checkpoint = upsert_checkpoint(&conn, user_id, "device1").unwrap();
        assert_eq!(checkpoint.device_id, "device1");
        assert_eq!(checkpoint.item_checkpoint, 0);
        assert_eq!(checkpoint.user_data_checkpoint, 0);

        // Upserting again should update the same checkpoint
        let checkpoint2 = upsert_checkpoint(&conn, user_id, "device1").unwrap();
        assert_eq!(checkpoint.id, checkpoint2.id);
    }

    #[test]
    fn test_get_checkpoint() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let (user_id, _) = create_test_setup(&conn);

        let created = upsert_checkpoint(&conn, user_id, "device1").unwrap();
        let found = get_checkpoint(&conn, created.id).unwrap();

        assert!(found.is_some());
        assert_eq!(found.unwrap().id, created.id);
    }

    #[test]
    fn test_get_checkpoint_by_device() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let (user_id, _) = create_test_setup(&conn);

        upsert_checkpoint(&conn, user_id, "device1").unwrap();
        let found = get_checkpoint_by_device(&conn, user_id, "device1").unwrap();

        assert!(found.is_some());
        assert_eq!(found.unwrap().device_id, "device1");
    }

    #[test]
    fn test_get_changed_items() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let (_, item_id) = create_test_setup(&conn);

        log_item_change(&conn, item_id, "added").unwrap();
        log_item_change(&conn, item_id, "updated").unwrap();

        let changes = get_changed_items(&conn, 0).unwrap();
        assert_eq!(changes.len(), 2);

        // Get changes after first log entry
        let changes = get_changed_items(&conn, changes[0].0).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].2, "updated");
    }

    #[test]
    fn test_get_removed_items() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let (_, item_id) = create_test_setup(&conn);

        log_item_change(&conn, item_id, "added").unwrap();
        log_item_change(&conn, item_id, "removed").unwrap();

        let removed = get_removed_items(&conn, 0).unwrap();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].1, item_id);
    }

    #[test]
    fn test_update_checkpoint() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let (user_id, _) = create_test_setup(&conn);

        let checkpoint = upsert_checkpoint(&conn, user_id, "device1").unwrap();
        update_checkpoint(&conn, checkpoint.id, 10, 5).unwrap();

        let updated = get_checkpoint(&conn, checkpoint.id).unwrap().unwrap();
        assert_eq!(updated.item_checkpoint, 10);
        assert_eq!(updated.user_data_checkpoint, 5);
    }

    #[test]
    fn test_delete_checkpoint() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let (user_id, _) = create_test_setup(&conn);

        let checkpoint = upsert_checkpoint(&conn, user_id, "device1").unwrap();
        let deleted = delete_checkpoint(&conn, checkpoint.id).unwrap();
        assert!(deleted);

        let found = get_checkpoint(&conn, checkpoint.id).unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_prune_sync_logs() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let (user_id, item_id) = create_test_setup(&conn);

        log_item_change(&conn, item_id, "added").unwrap();
        log_user_data_change(&conn, user_id, item_id).unwrap();

        // Pruning with large days value should delete nothing
        let deleted = prune_sync_logs(&conn, 365).unwrap();
        assert_eq!(deleted, 0);

        // Pruning with 0 days should delete everything
        let deleted = prune_sync_logs(&conn, 0).unwrap();
        assert_eq!(deleted, 2);
    }
}
