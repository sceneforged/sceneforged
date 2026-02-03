//! In-memory segment map cache.
//!
//! Caches parsed SegmentMaps for files to avoid re-parsing on every request.

use dashmap::DashMap;
use sceneforged_media::segment_map::SegmentMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Entry in the segment cache.
struct CacheEntry {
    segment_map: Arc<SegmentMap>,
    file_path: PathBuf,
    last_accessed: Instant,
    file_modified: std::time::SystemTime,
}

/// Thread-safe cache for segment maps.
pub struct SegmentCache {
    entries: DashMap<String, CacheEntry>,
    max_entries: usize,
    ttl: Duration,
}

impl SegmentCache {
    /// Create a new segment cache.
    pub fn new(max_entries: usize, ttl_secs: u64) -> Self {
        Self {
            entries: DashMap::new(),
            max_entries,
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    /// Get a segment map from cache or compute it.
    pub fn get_or_insert<F>(
        &self,
        media_file_id: &str,
        file_path: &Path,
        compute: F,
    ) -> Option<Arc<SegmentMap>>
    where
        F: FnOnce(&Path) -> Option<SegmentMap>,
    {
        // Check if we have a valid cached entry
        if let Some(mut entry) = self.entries.get_mut(media_file_id) {
            // Validate cache entry
            if self.is_entry_valid(&entry, file_path) {
                entry.last_accessed = Instant::now();
                return Some(Arc::clone(&entry.segment_map));
            }
            // Entry is stale, remove it
            drop(entry);
            self.entries.remove(media_file_id);
        }

        // Compute new segment map
        let segment_map = compute(file_path)?;
        let segment_map = Arc::new(segment_map);

        // Get file modification time
        let file_modified = std::fs::metadata(file_path)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| std::time::SystemTime::now());

        // Insert into cache
        let entry = CacheEntry {
            segment_map: Arc::clone(&segment_map),
            file_path: file_path.to_owned(),
            last_accessed: Instant::now(),
            file_modified,
        };

        // Evict old entries if at capacity
        if self.entries.len() >= self.max_entries {
            self.evict_oldest();
        }

        self.entries.insert(media_file_id.to_string(), entry);
        Some(segment_map)
    }

    /// Get a segment map if it exists in cache.
    pub fn get(&self, media_file_id: &str) -> Option<Arc<SegmentMap>> {
        self.entries.get_mut(media_file_id).map(|mut entry| {
            entry.last_accessed = Instant::now();
            Arc::clone(&entry.segment_map)
        })
    }

    /// Remove an entry from the cache.
    pub fn remove(&self, media_file_id: &str) {
        self.entries.remove(media_file_id);
    }

    /// Clear all entries.
    pub fn clear(&self) {
        self.entries.clear();
    }

    /// Get the number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove expired entries.
    pub fn cleanup_expired(&self) {
        let now = Instant::now();
        self.entries
            .retain(|_, entry| now.duration_since(entry.last_accessed) < self.ttl);
    }

    fn is_entry_valid(&self, entry: &CacheEntry, file_path: &Path) -> bool {
        // Check if file path matches
        if entry.file_path != file_path {
            return false;
        }

        // Check if TTL has expired
        if entry.last_accessed.elapsed() >= self.ttl {
            return false;
        }

        // Check if file has been modified
        if let Ok(metadata) = std::fs::metadata(file_path) {
            if let Ok(modified) = metadata.modified() {
                return modified == entry.file_modified;
            }
        }

        // If we can't check modification time, assume valid
        true
    }

    fn evict_oldest(&self) {
        // Find the oldest entry
        let oldest = self
            .entries
            .iter()
            .min_by_key(|entry| entry.last_accessed)
            .map(|entry| entry.key().clone());

        if let Some(key) = oldest {
            self.entries.remove(&key);
        }
    }
}

impl Default for SegmentCache {
    fn default() -> Self {
        // Default: 100 entries, 1 hour TTL
        Self::new(100, 3600)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sceneforged_media::segment_map::Segment;

    fn create_test_segment_map() -> SegmentMap {
        SegmentMap {
            timescale: 90000,
            duration_secs: 10.0,
            target_duration_secs: 6.0,
            max_segment_duration_secs: 6.0,
            segments: vec![Segment {
                index: 0,
                start_sample: 0,
                end_sample: 10,
                duration_secs: 10.0,
                start_time_secs: 0.0,
                byte_ranges: vec![(0, 1000)],
                audio_byte_ranges: Vec::new(),
                audio_start_sample: None,
                audio_end_sample: None,
                moof_data: None,
            }],
            sample_count: 10,
            init_segment: None,
        }
    }

    #[test]
    fn test_cache_insert_and_get() {
        let cache = SegmentCache::new(10, 3600);
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_cache.mp4");
        std::fs::write(&temp_file, b"test").unwrap();

        let result =
            cache.get_or_insert("test_id", &temp_file, |_| Some(create_test_segment_map()));

        assert!(result.is_some());
        assert_eq!(cache.len(), 1);

        // Second call should hit cache
        let cached = cache.get("test_id");
        assert!(cached.is_some());

        std::fs::remove_file(&temp_file).ok();
    }

    #[test]
    fn test_cache_remove() {
        let cache = SegmentCache::new(10, 3600);
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_cache_remove.mp4");
        std::fs::write(&temp_file, b"test").unwrap();

        cache.get_or_insert("test_id", &temp_file, |_| Some(create_test_segment_map()));

        assert_eq!(cache.len(), 1);
        cache.remove("test_id");
        assert_eq!(cache.len(), 0);

        std::fs::remove_file(&temp_file).ok();
    }

    #[test]
    fn test_cache_eviction() {
        let cache = SegmentCache::new(2, 3600);
        let temp_dir = std::env::temp_dir();

        for i in 0..3 {
            let temp_file = temp_dir.join(format!("test_evict_{}.mp4", i));
            std::fs::write(&temp_file, b"test").unwrap();

            cache.get_or_insert(&format!("id_{}", i), &temp_file, |_| {
                Some(create_test_segment_map())
            });

            std::fs::remove_file(&temp_file).ok();
        }

        // Should have evicted oldest, keeping only 2
        assert_eq!(cache.len(), 2);
    }
}
