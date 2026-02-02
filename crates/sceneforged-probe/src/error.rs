//! Error types for sceneforged-probe

use std::path::PathBuf;

/// Errors that can occur during video probing
#[derive(Debug, thiserror::Error)]
pub enum VideoProbeError {
    /// File not found
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Unsupported container format
    #[error("Unsupported container format: {0}")]
    UnsupportedContainer(String),

    /// Failed to parse container
    #[error("Failed to parse container: {0}")]
    ContainerParse(String),

    /// Failed to parse video codec
    #[error("Failed to parse video codec: {0}")]
    CodecParse(String),

    /// No video tracks found
    #[error("No video tracks found in file")]
    NoVideoTracks,

    /// Invalid data encountered
    #[error("Invalid data: {0}")]
    InvalidData(String),
}
