//! Unified error type for the sceneforged application.
//!
//! All crates funnel their failures into [`Error`], which carries enough context
//! for API handlers to derive an HTTP status code via [`Error::http_status`].

use std::fmt;

/// Unified error type covering all failure modes in sceneforged.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The requested entity could not be found.
    #[error("{entity} not found: {id}")]
    NotFound {
        /// The kind of entity (e.g. "item", "library").
        entity: String,
        /// The identifier that was looked up.
        id: String,
    },

    /// The caller is not authenticated.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// The caller lacks permission for the requested action.
    #[error("Forbidden: {0}")]
    Forbidden(String),

    /// Request data failed validation.
    #[error("Validation error: {0}")]
    Validation(String),

    /// A conflicting resource already exists.
    #[error("Conflict: {0}")]
    Conflict(String),

    /// A database operation failed.
    #[error("Database error: {source}")]
    Database {
        /// The underlying database error.
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// An I/O operation failed.
    #[error("IO error: {source}")]
    Io {
        /// The underlying I/O error.
        #[from]
        source: std::io::Error,
    },

    /// An external tool (ffmpeg, mkvmerge, etc.) returned an error.
    #[error("Tool error [{tool}]: {message}")]
    Tool {
        /// Name of the tool that failed.
        tool: String,
        /// Human-readable error description.
        message: String,
    },

    /// Media probing failed.
    #[error("Probe error: {0}")]
    Probe(String),

    /// A pipeline step failed.
    #[error("Pipeline error [{step}]: {message}")]
    Pipeline {
        /// The pipeline step that failed.
        step: String,
        /// Human-readable error description.
        message: String,
    },

    /// Catch-all for unexpected internal errors.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl Error {
    /// Map this error to an appropriate HTTP status code.
    pub fn http_status(&self) -> u16 {
        match self {
            Error::NotFound { .. } => 404,
            Error::Unauthorized(_) => 401,
            Error::Forbidden(_) => 403,
            Error::Validation(_) => 400,
            Error::Conflict(_) => 409,
            Error::Database { .. } => 500,
            Error::Io { .. } => 500,
            Error::Tool { .. } => 502,
            Error::Probe(_) => 422,
            Error::Pipeline { .. } => 500,
            Error::Internal(_) => 500,
        }
    }

    /// Convenience constructor for [`Error::NotFound`].
    pub fn not_found(entity: impl Into<String>, id: impl fmt::Display) -> Self {
        Error::NotFound {
            entity: entity.into(),
            id: id.to_string(),
        }
    }

    /// Convenience constructor for [`Error::Database`].
    pub fn database(source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Error::Database {
            source: source.into(),
        }
    }

    /// Convenience constructor for [`Error::Tool`].
    pub fn tool(tool: impl Into<String>, message: impl Into<String>) -> Self {
        Error::Tool {
            tool: tool.into(),
            message: message.into(),
        }
    }

    /// Convenience constructor for [`Error::Pipeline`].
    pub fn pipeline(step: impl Into<String>, message: impl Into<String>) -> Self {
        Error::Pipeline {
            step: step.into(),
            message: message.into(),
        }
    }
}

/// Result alias using the crate-level [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_display() {
        let err = Error::not_found("item", "abc-123");
        assert_eq!(err.to_string(), "item not found: abc-123");
        assert_eq!(err.http_status(), 404);
    }

    #[test]
    fn unauthorized_display() {
        let err = Error::Unauthorized("bad token".into());
        assert_eq!(err.to_string(), "Unauthorized: bad token");
        assert_eq!(err.http_status(), 401);
    }

    #[test]
    fn forbidden_display() {
        let err = Error::Forbidden("admin only".into());
        assert_eq!(err.to_string(), "Forbidden: admin only");
        assert_eq!(err.http_status(), 403);
    }

    #[test]
    fn validation_display() {
        let err = Error::Validation("name is required".into());
        assert_eq!(err.to_string(), "Validation error: name is required");
        assert_eq!(err.http_status(), 400);
    }

    #[test]
    fn conflict_display() {
        let err = Error::Conflict("library already exists".into());
        assert_eq!(err.to_string(), "Conflict: library already exists");
        assert_eq!(err.http_status(), 409);
    }

    #[test]
    fn database_display() {
        let err = Error::database("connection refused");
        assert!(err.to_string().contains("connection refused"));
        assert_eq!(err.http_status(), 500);
    }

    #[test]
    fn io_from_std() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err = Error::from(io_err);
        assert!(matches!(err, Error::Io { .. }));
        assert_eq!(err.http_status(), 500);
    }

    #[test]
    fn tool_display() {
        let err = Error::tool("ffmpeg", "exit code 1");
        assert_eq!(err.to_string(), "Tool error [ffmpeg]: exit code 1");
        assert_eq!(err.http_status(), 502);
    }

    #[test]
    fn probe_display() {
        let err = Error::Probe("corrupt header".into());
        assert_eq!(err.to_string(), "Probe error: corrupt header");
        assert_eq!(err.http_status(), 422);
    }

    #[test]
    fn pipeline_display() {
        let err = Error::pipeline("remux", "mkvmerge failed");
        assert_eq!(
            err.to_string(),
            "Pipeline error [remux]: mkvmerge failed"
        );
        assert_eq!(err.http_status(), 500);
    }

    #[test]
    fn internal_display() {
        let err = Error::Internal("unexpected state".into());
        assert_eq!(err.to_string(), "Internal error: unexpected state");
        assert_eq!(err.http_status(), 500);
    }

    #[test]
    fn result_alias() {
        fn ok_fn() -> Result<i32> {
            Ok(42)
        }
        assert_eq!(ok_fn().unwrap(), 42);

        fn err_fn() -> Result<i32> {
            Err(Error::Internal("boom".into()))
        }
        assert!(err_fn().is_err());
    }
}
