//! Typed ID wrappers for type safety across sceneforged.
//!
//! This module provides newtype wrappers around UUIDs to prevent mixing different
//! types of identifiers (e.g., using a UserId where an ItemId is expected).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UserId(Uuid);

impl UserId {
    /// Generate a new random user ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for UserId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for UserId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<UserId> for Uuid {
    fn from(id: UserId) -> Self {
        id.0
    }
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a library item (movie, episode, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemId(Uuid);

impl ItemId {
    /// Generate a new random item ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ItemId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for ItemId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<ItemId> for Uuid {
    fn from(id: ItemId) -> Self {
        id.0
    }
}

impl std::fmt::Display for ItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a media library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LibraryId(Uuid);

impl LibraryId {
    /// Generate a new random library ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for LibraryId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for LibraryId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<LibraryId> for Uuid {
    fn from(id: LibraryId) -> Self {
        id.0
    }
}

impl std::fmt::Display for LibraryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a playback session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(Uuid);

impl SessionId {
    /// Generate a new random session ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for SessionId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<SessionId> for Uuid {
    fn from(id: SessionId) -> Self {
        id.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for an image/artwork resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ImageId(Uuid);

impl ImageId {
    /// Generate a new random image ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ImageId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for ImageId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<ImageId> for Uuid {
    fn from(id: ImageId) -> Self {
        id.0
    }
}

impl std::fmt::Display for ImageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a playback position checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CheckpointId(Uuid);

impl CheckpointId {
    /// Generate a new random checkpoint ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for CheckpointId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for CheckpointId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<CheckpointId> for Uuid {
    fn from(id: CheckpointId) -> Self {
        id.0
    }
}

impl std::fmt::Display for CheckpointId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a media file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MediaFileId(Uuid);

impl MediaFileId {
    /// Generate a new random media file ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for MediaFileId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for MediaFileId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<MediaFileId> for Uuid {
    fn from(id: MediaFileId) -> Self {
        id.0
    }
}

impl std::fmt::Display for MediaFileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_id_creation() {
        let id1 = UserId::new();
        let id2 = UserId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_user_id_from_uuid() {
        let uuid = Uuid::new_v4();
        let user_id = UserId::from(uuid);
        let uuid_back: Uuid = user_id.into();
        assert_eq!(uuid, uuid_back);
    }

    #[test]
    fn test_item_id_serialization() {
        let id = ItemId::new();
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: ItemId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_library_id_display() {
        let id = LibraryId::new();
        let display = format!("{}", id);
        assert!(!display.is_empty());
    }

    #[test]
    fn test_different_id_types() {
        let uuid = Uuid::new_v4();
        let _user_id = UserId::from(uuid);
        let _item_id = ItemId::from(uuid);
        // Type system prevents mixing these at compile time
    }

    #[test]
    fn test_session_id_default() {
        let id1 = SessionId::default();
        let id2 = SessionId::default();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_image_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        let id = ImageId::new();
        set.insert(id);
        assert!(set.contains(&id));
    }

    #[test]
    fn test_checkpoint_id_clone() {
        let id = CheckpointId::new();
        let cloned = id;
        assert_eq!(id, cloned);
    }

    #[test]
    fn test_media_file_id_creation() {
        let id1 = MediaFileId::new();
        let id2 = MediaFileId::new();
        assert_ne!(id1, id2);
    }
}
