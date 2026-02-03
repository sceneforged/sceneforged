//! Provider registry for managing multiple [`MetadataProvider`] implementations.
//!
//! The [`ProviderRegistry`] aggregates metadata providers and exposes a unified
//! interface for searching across all configured backends. Results from multiple
//! providers are merged, deduplicated, and sorted by confidence.

use std::sync::Arc;

use anyhow::Result;

use super::provider::{MetadataProvider, SearchResult};

/// A registry that manages multiple [`MetadataProvider`] implementations.
///
/// Providers are stored in registration order. When performing a search the
/// registry queries every *available* provider, merges the results, deduplicates
/// entries that share the same title and year (keeping the highest-confidence
/// hit), and returns them sorted by descending confidence.
///
/// # Examples
///
/// ```rust,ignore
/// use std::sync::Arc;
/// use sceneforged::metadata::registry::ProviderRegistry;
///
/// let mut registry = ProviderRegistry::new();
/// registry.register(Arc::new(my_provider));
///
/// let results = registry.search_movie("Interstellar", Some(2014)).await?;
/// ```
pub struct ProviderRegistry {
    providers: Vec<Arc<dyn MetadataProvider>>,
}

impl ProviderRegistry {
    /// Create an empty registry with no providers.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a new metadata provider.
    ///
    /// Providers are stored in the order they are registered. The first
    /// available provider becomes the *primary* provider.
    pub fn register(&mut self, provider: Arc<dyn MetadataProvider>) {
        self.providers.push(provider);
    }

    /// Return references to all providers that are currently available
    /// (i.e. configured with valid credentials).
    pub fn available(&self) -> Vec<&dyn MetadataProvider> {
        self.providers
            .iter()
            .filter(|p| p.is_available())
            .map(|p| p.as_ref())
            .collect()
    }

    /// Return the first available provider, or `None` if no providers are
    /// configured / available.
    pub fn primary(&self) -> Option<&dyn MetadataProvider> {
        self.providers
            .iter()
            .find(|p| p.is_available())
            .map(|p| p.as_ref())
    }

    /// Look up a provider by its [`MetadataProvider::name`].
    ///
    /// Returns `None` if no provider with the given name has been registered.
    pub fn get(&self, name: &str) -> Option<&dyn MetadataProvider> {
        self.providers
            .iter()
            .find(|p| p.name() == name)
            .map(|p| p.as_ref())
    }

    /// Search for movies across all available providers.
    ///
    /// Results are merged and deduplicated: when multiple providers return
    /// results with the same title (case-insensitive) **and** year, only the
    /// entry with the highest confidence is kept. The final list is sorted by
    /// descending confidence.
    pub async fn search_movie(
        &self,
        title: &str,
        year: Option<u16>,
    ) -> Result<Vec<SearchResult>> {
        let available = self.available();
        if available.is_empty() {
            return Ok(Vec::new());
        }

        // Collect results from every available provider.
        let mut all_results: Vec<SearchResult> = Vec::new();
        for provider in &available {
            match provider.search_movie(title, year).await {
                Ok(results) => all_results.extend(results),
                // Log-worthy in production, but we don't want one failing
                // provider to prevent the others from contributing results.
                Err(_) => continue,
            }
        }

        // Deduplicate: for entries sharing the same (lowercased title, year)
        // keep only the one with the highest confidence.
        let mut seen = std::collections::HashMap::<(String, Option<u16>), usize>::new();
        let mut deduped: Vec<SearchResult> = Vec::new();

        for result in all_results {
            let key = (result.title.to_lowercase(), result.year);
            if let Some(&idx) = seen.get(&key) {
                if result.confidence > deduped[idx].confidence {
                    deduped[idx] = result;
                }
            } else {
                seen.insert(key, deduped.len());
                deduped.push(result);
            }
        }

        // Sort by confidence descending.
        deduped.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(deduped)
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::provider::{MediaImages, MediaMetadata};
    use async_trait::async_trait;

    /// A minimal stub provider used for testing.
    struct StubProvider {
        provider_name: &'static str,
        available: bool,
        results: Vec<SearchResult>,
    }

    #[async_trait]
    impl MetadataProvider for StubProvider {
        fn name(&self) -> &'static str {
            self.provider_name
        }

        fn is_available(&self) -> bool {
            self.available
        }

        async fn search_movie(
            &self,
            _title: &str,
            _year: Option<u16>,
        ) -> Result<Vec<SearchResult>> {
            Ok(self.results.clone())
        }

        async fn search_tv(&self, _title: &str) -> Result<Vec<SearchResult>> {
            Ok(Vec::new())
        }

        async fn get_movie_metadata(&self, _provider_id: &str) -> Result<MediaMetadata> {
            anyhow::bail!("not implemented")
        }

        async fn get_tv_metadata(&self, _provider_id: &str) -> Result<MediaMetadata> {
            anyhow::bail!("not implemented")
        }

        async fn get_movie_images(&self, _provider_id: &str) -> Result<MediaImages> {
            Ok(MediaImages {
                posters: Vec::new(),
                backdrops: Vec::new(),
                logos: Vec::new(),
            })
        }

        async fn get_tv_images(&self, _provider_id: &str) -> Result<MediaImages> {
            Ok(MediaImages {
                posters: Vec::new(),
                backdrops: Vec::new(),
                logos: Vec::new(),
            })
        }
    }

    fn make_result(title: &str, year: Option<u16>, confidence: f64, provider: &str) -> SearchResult {
        SearchResult {
            id: format!("{provider}-{title}"),
            title: title.to_string(),
            year,
            overview: None,
            confidence,
            provider_name: provider.to_string(),
            poster_path: None,
        }
    }

    #[test]
    fn empty_registry() {
        let registry = ProviderRegistry::new();
        assert!(registry.available().is_empty());
        assert!(registry.primary().is_none());
        assert!(registry.get("tmdb").is_none());
    }

    #[test]
    fn register_and_lookup() {
        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(StubProvider {
            provider_name: "tmdb",
            available: true,
            results: Vec::new(),
        }));
        registry.register(Arc::new(StubProvider {
            provider_name: "omdb",
            available: false,
            results: Vec::new(),
        }));

        assert_eq!(registry.available().len(), 1);
        assert_eq!(registry.primary().unwrap().name(), "tmdb");
        assert!(registry.get("tmdb").is_some());
        assert!(registry.get("omdb").is_some()); // registered but not available
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn primary_returns_first_available() {
        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(StubProvider {
            provider_name: "offline",
            available: false,
            results: Vec::new(),
        }));
        registry.register(Arc::new(StubProvider {
            provider_name: "online",
            available: true,
            results: Vec::new(),
        }));

        assert_eq!(registry.primary().unwrap().name(), "online");
    }

    #[tokio::test]
    async fn search_movie_merges_providers() {
        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(StubProvider {
            provider_name: "provider_a",
            available: true,
            results: vec![make_result("Interstellar", Some(2014), 0.95, "provider_a")],
        }));
        registry.register(Arc::new(StubProvider {
            provider_name: "provider_b",
            available: true,
            results: vec![make_result("The Martian", Some(2015), 0.90, "provider_b")],
        }));

        let results = registry.search_movie("test", None).await.unwrap();
        assert_eq!(results.len(), 2);
        // Sorted by confidence descending.
        assert_eq!(results[0].title, "Interstellar");
        assert_eq!(results[1].title, "The Martian");
    }

    #[tokio::test]
    async fn search_movie_deduplicates_by_title_year() {
        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(StubProvider {
            provider_name: "low",
            available: true,
            results: vec![make_result("Interstellar", Some(2014), 0.80, "low")],
        }));
        registry.register(Arc::new(StubProvider {
            provider_name: "high",
            available: true,
            results: vec![make_result("Interstellar", Some(2014), 0.99, "high")],
        }));

        let results = registry.search_movie("Interstellar", Some(2014)).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].provider_name, "high");
        assert!((results[0].confidence - 0.99).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn search_movie_skips_unavailable_providers() {
        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(StubProvider {
            provider_name: "offline",
            available: false,
            results: vec![make_result("Ghost", Some(1990), 0.99, "offline")],
        }));
        registry.register(Arc::new(StubProvider {
            provider_name: "online",
            available: true,
            results: vec![make_result("Real Result", Some(2020), 0.85, "online")],
        }));

        let results = registry.search_movie("test", None).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Real Result");
    }

    #[tokio::test]
    async fn search_movie_empty_when_no_providers() {
        let registry = ProviderRegistry::new();
        let results = registry.search_movie("anything", None).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn search_movie_case_insensitive_dedup() {
        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(StubProvider {
            provider_name: "a",
            available: true,
            results: vec![make_result("interstellar", Some(2014), 0.70, "a")],
        }));
        registry.register(Arc::new(StubProvider {
            provider_name: "b",
            available: true,
            results: vec![make_result("Interstellar", Some(2014), 0.95, "b")],
        }));

        let results = registry.search_movie("Interstellar", None).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].provider_name, "b");
    }
}
