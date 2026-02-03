//! Metadata provider system for enriching media items with external data.
//!
//! This module defines a generic [`MetadataProvider`] trait and supporting types
//! that allow Sceneforged to fetch metadata and artwork from external services
//! such as TMDB, TVDb, OMDb, and Fanart.tv.
//!
//! # Module layout
//!
//! - [`provider`] -- Trait definition and shared data types.
//! - `providers` -- Concrete provider implementations (TMDB, etc.) *(planned)*.
//! - [`registry`] -- Provider registry for multi-source lookups.
//! - [`enrichment`] -- Logic for enriching media items with fetched metadata.
//! - [`queue`] -- Background enrichment queue for async metadata processing.

pub mod provider;
pub mod registry;

// Future submodules (uncomment as they are implemented):
pub mod providers;
pub mod enrichment;
pub mod queue;

// Re-export the images module from crate root so enrichment and queue
// submodules can reference it as `super::images`.
pub(crate) use crate::images;

pub use provider::{ImageInfo, MediaImages, MediaMetadata, MetadataProvider, SearchResult};
pub use queue::{EnrichmentJob, EnrichmentQueue};
pub use registry::ProviderRegistry;
