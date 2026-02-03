//! Concrete metadata provider implementations.
//!
//! Each submodule wraps a single external API and implements the
//! [`MetadataProvider`](super::MetadataProvider) trait.

pub mod tmdb;

pub use tmdb::TmdbProvider;
