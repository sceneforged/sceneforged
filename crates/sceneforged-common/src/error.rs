//! Common error types used throughout sceneforged.
//!
//! This module provides a unified error type that covers common failure cases
//! such as not found, unauthorized access, database errors, and I/O failures.

/// Common error type for sceneforged.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The requested item was not found.
    #[error("Item not found: {0}")]
    NotFound(String),

    /// The user is not authenticated.
    #[error("Unauthorized")]
    Unauthorized,

    /// The user does not have permission to access the resource.
    #[error("Forbidden")]
    Forbidden,

    /// A database operation failed.
    #[error("Database error: {0}")]
    Database(String),

    /// An I/O operation failed.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid input was provided.
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// An internal error occurred.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl Error {
    /// Create a new NotFound error.
    pub fn not_found<S: Into<String>>(msg: S) -> Self {
        Self::NotFound(msg.into())
    }

    /// Create a new Database error.
    pub fn database<S: Into<String>>(msg: S) -> Self {
        Self::Database(msg.into())
    }

    /// Create a new InvalidInput error.
    pub fn invalid_input<S: Into<String>>(msg: S) -> Self {
        Self::InvalidInput(msg.into())
    }

    /// Create a new Internal error.
    pub fn internal<S: Into<String>>(msg: S) -> Self {
        Self::Internal(msg.into())
    }

    /// Create a new Io error from a message (for external API errors, etc).
    pub fn io<S: Into<String>>(msg: S) -> Self {
        Self::Io(std::io::Error::other(msg.into()))
    }

    /// Alias for invalid_input (for consistency).
    pub fn validation<S: Into<String>>(msg: S) -> Self {
        Self::InvalidInput(msg.into())
    }
}

/// Result type alias using the common Error type.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::not_found("test item");
        assert_eq!(err.to_string(), "Item not found: test item");

        let err = Error::Unauthorized;
        assert_eq!(err.to_string(), "Unauthorized");

        let err = Error::Forbidden;
        assert_eq!(err.to_string(), "Forbidden");

        let err = Error::database("connection failed");
        assert_eq!(err.to_string(), "Database error: connection failed");

        let err = Error::invalid_input("bad format");
        assert_eq!(err.to_string(), "Invalid input: bad format");

        let err = Error::internal("unexpected state");
        assert_eq!(err.to_string(), "Internal error: unexpected state");
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = Error::from(io_err);
        assert!(matches!(err, Error::Io(_)));
    }

    #[test]
    fn test_result_type() {
        fn test_fn() -> Result<i32> {
            Ok(42)
        }
        assert_eq!(test_fn().unwrap(), 42);

        fn error_fn() -> Result<i32> {
            Err(Error::Unauthorized)
        }
        assert!(error_fn().is_err());
    }

    #[test]
    fn test_error_constructors() {
        let err = Error::not_found("item");
        assert!(matches!(err, Error::NotFound(_)));

        let err = Error::database("query failed");
        assert!(matches!(err, Error::Database(_)));

        let err = Error::invalid_input("bad data");
        assert!(matches!(err, Error::InvalidInput(_)));

        let err = Error::internal("bug");
        assert!(matches!(err, Error::Internal(_)));
    }

    #[test]
    fn test_error_string_into() {
        let err = Error::not_found(String::from("test"));
        assert_eq!(err.to_string(), "Item not found: test");

        let err = Error::not_found("test");
        assert_eq!(err.to_string(), "Item not found: test");
    }
}
