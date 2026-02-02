//! Database query modules.
//!
//! This module organizes all database operations into logical groups:
//! - users: User CRUD and authentication
//! - auth_tokens: Authentication token management
//! - libraries: Library management
//! - items: Item CRUD, hierarchy, and search
//! - media_files: Media file management (source, universal, extra)
//! - conversion_jobs: Conversion job tracking
//! - playback: User playback data and favorites
//! - sync: InfuseSync delta sync operations

pub mod auth_tokens;
pub mod conversion_jobs;
pub mod items;
pub mod libraries;
pub mod media_files;
pub mod playback;
pub mod sync;
pub mod users;
