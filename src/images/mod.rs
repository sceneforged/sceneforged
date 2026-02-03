//! Image storage and management module.
//!
//! This module provides local image storage with automatic size variant generation
//! and database record management. It coordinates filesystem storage with the
//! database layer from `sceneforged_db`.

mod service;
mod storage;

pub use service::ImageService;
pub use storage::{ImageSize, ImageStorage, StoredImage};
