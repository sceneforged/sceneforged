//! Sceneforged-DB: Database schema, migrations, and query operations
//!
//! This crate provides database functionality for sceneforged using SQLite
//! with rusqlite and r2d2 connection pooling.
//!
//! # Modules
//!
//! - `migrations` - Database schema migrations
//! - `pool` - Connection pool management
//! - `models` - Rust models matching database schema
//! - `queries` - Database query operations
//!
//! # Example
//!
//! ```no_run
//! use sceneforged_db::pool::{init_pool, get_conn};
//! use sceneforged_db::queries::users;
//!
//! let pool = init_pool("/var/lib/sceneforged/db.sqlite").unwrap();
//! let conn = get_conn(&pool).unwrap();
//!
//! let user = users::create_user(&conn, "admin", "hash", true).unwrap();
//! println!("Created user: {}", user.username);
//! ```

pub mod migrations;
pub mod models;
pub mod pool;
pub mod queries;
