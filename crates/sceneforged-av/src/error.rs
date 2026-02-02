//! Error types for sceneforged-av.

use std::path::PathBuf;

/// Result type alias using our Error type.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during media processing.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A required external tool is not available.
    #[error("tool not found: {tool}")]
    ToolNotFound { tool: String },

    /// An external tool failed to execute.
    #[error("tool execution failed: {tool}: {message}")]
    ToolFailed { tool: String, message: String },

    /// Failed to parse tool output.
    #[error("failed to parse {tool} output: {message}")]
    ParseError { tool: String, message: String },

    /// The specified file was not found.
    #[error("file not found: {}", path.display())]
    FileNotFound { path: PathBuf },

    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parsing error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Invalid input provided.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// Unsupported operation or format.
    #[error("unsupported: {0}")]
    Unsupported(String),

    /// Workspace error.
    #[error("workspace error: {0}")]
    Workspace(String),

    /// Dolby Vision processing error.
    #[cfg(feature = "dovi")]
    #[error("Dolby Vision error: {0}")]
    DolbyVision(String),

    /// FFmpeg library error.
    #[cfg(feature = "native-ffmpeg")]
    #[error("FFmpeg error: {0}")]
    FFmpeg(String),
}

impl Error {
    /// Create a tool not found error.
    pub fn tool_not_found(tool: impl Into<String>) -> Self {
        Self::ToolNotFound { tool: tool.into() }
    }

    /// Create a tool execution failed error.
    pub fn tool_failed(tool: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ToolFailed {
            tool: tool.into(),
            message: message.into(),
        }
    }

    /// Create a parse error.
    pub fn parse_error(tool: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ParseError {
            tool: tool.into(),
            message: message.into(),
        }
    }

    /// Create a file not found error.
    pub fn file_not_found(path: impl Into<PathBuf>) -> Self {
        Self::FileNotFound { path: path.into() }
    }
}

#[cfg(feature = "native-ffmpeg")]
impl From<ffmpeg_the_third::Error> for Error {
    fn from(err: ffmpeg_the_third::Error) -> Self {
        Error::FFmpeg(err.to_string())
    }
}
