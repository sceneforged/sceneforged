//! Error types for sceneforged-media.

use std::io;
use thiserror::Error;

/// Result type for sceneforged-media operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Error type for sceneforged-media operations.
#[derive(Debug, Error)]
pub enum Error {
    /// I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Invalid MP4 file structure.
    #[error("Invalid MP4: {0}")]
    InvalidMp4(String),

    /// Missing required atom in MP4 file.
    #[error("Missing required atom: {0}")]
    MissingAtom(&'static str),

    /// Unsupported feature or codec.
    #[error("Unsupported: {0}")]
    Unsupported(String),

    /// Buffer too small for operation.
    #[error("Buffer underflow: need {need} bytes, have {have}")]
    BufferUnderflow { need: usize, have: usize },

    /// Invalid segment index.
    #[error("Invalid segment index: {index} (max: {max})")]
    InvalidSegmentIndex { index: u32, max: u32 },
}

impl Error {
    /// Create an invalid MP4 error.
    pub fn invalid_mp4(msg: impl Into<String>) -> Self {
        Self::InvalidMp4(msg.into())
    }

    /// Create an unsupported error.
    pub fn unsupported(msg: impl Into<String>) -> Self {
        Self::Unsupported(msg.into())
    }
}
