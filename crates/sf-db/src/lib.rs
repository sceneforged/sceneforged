//! sf-db: database access and persistence layer.
//!
//! This crate provides SQLite-backed storage with connection pooling,
//! embedded migrations, typed models, and query modules for all
//! sceneforged entities.

pub mod migrations;
pub mod models;
pub mod pool;
pub mod queries;
