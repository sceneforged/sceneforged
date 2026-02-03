//! Typed ID wrappers providing compile-time safety for entity identifiers.
//!
//! Each ID type is a newtype over `Uuid`, preventing accidental misuse
//! (e.g., passing a `UserId` where a `LibraryId` is expected).

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Generate a newtype ID wrapper over `Uuid`.
///
/// The macro produces a struct with:
/// - `new()` to create a random v4 UUID
/// - `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `Serialize`, `Deserialize`
/// - `Display` and `FromStr` delegating to the inner UUID
/// - `From<Uuid>` and `Into<Uuid>` conversions
/// - `Default` that generates a new random ID
macro_rules! typed_id {
    ($($(#[doc = $doc:expr])* $name:ident),+ $(,)?) => {
        $(
            $(#[doc = $doc])*
            #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
            #[serde(transparent)]
            pub struct $name(Uuid);

            impl $name {
                /// Create a new random ID.
                #[must_use]
                pub fn new() -> Self {
                    Self(Uuid::new_v4())
                }

                /// Return the inner UUID value.
                #[must_use]
                pub fn as_uuid(&self) -> &Uuid {
                    &self.0
                }
            }

            impl Default for $name {
                fn default() -> Self {
                    Self::new()
                }
            }

            impl fmt::Display for $name {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    write!(f, "{}", self.0)
                }
            }

            impl FromStr for $name {
                type Err = uuid::Error;

                fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
                    Uuid::parse_str(s).map(Self)
                }
            }

            impl From<Uuid> for $name {
                fn from(uuid: Uuid) -> Self {
                    Self(uuid)
                }
            }

            impl From<$name> for Uuid {
                fn from(id: $name) -> Self {
                    id.0
                }
            }
        )+
    };
}

typed_id! {
    /// Unique identifier for a processing job.
    JobId,
    /// Unique identifier for a library item (movie, episode, etc.).
    ItemId,
    /// Unique identifier for a media library.
    LibraryId,
    /// Unique identifier for a media file.
    MediaFileId,
    /// Unique identifier for a user.
    UserId,
    /// Unique identifier for an authentication session.
    SessionId,
    /// Unique identifier for an image/artwork resource.
    ImageId,
    /// Unique identifier for a conversion job.
    ConversionJobId,
    /// Unique identifier for a processing rule.
    RuleId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let a = JobId::new();
        let b = JobId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn roundtrip_uuid() {
        let uuid = Uuid::new_v4();
        let id = ItemId::from(uuid);
        let back: Uuid = id.into();
        assert_eq!(uuid, back);
    }

    #[test]
    fn display_and_from_str() {
        let id = LibraryId::new();
        let s = id.to_string();
        let parsed: LibraryId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn serde_roundtrip() {
        let id = MediaFileId::new();
        let json = serde_json::to_string(&id).unwrap();
        let back: MediaFileId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn hash_set_usage() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        let id = UserId::new();
        set.insert(id);
        assert!(set.contains(&id));
    }

    #[test]
    fn default_generates_unique() {
        let a = SessionId::default();
        let b = SessionId::default();
        assert_ne!(a, b);
    }

    #[test]
    fn as_uuid_reference() {
        let id = ImageId::new();
        let uuid_ref = id.as_uuid();
        let uuid_owned: Uuid = id.into();
        assert_eq!(*uuid_ref, uuid_owned);
    }

    #[test]
    fn invalid_from_str() {
        let result = ConversionJobId::from_str("not-a-uuid");
        assert!(result.is_err());
    }

    #[test]
    fn copy_semantics() {
        let id = RuleId::new();
        let copied = id;
        assert_eq!(id, copied);
    }
}
