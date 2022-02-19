use std::collections::HashMap;

use pandark_types::FetchArtifact;

/// In-memory response cache keyed by artifact cache keys.
#[derive(Debug, Clone, Default)]
pub struct ResponseCache {
    entries: HashMap<String, FetchArtifact>,
}

impl ResponseCache {
    /// Empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Lookup a cached artifact.
    pub fn get(&self, cache_key: &str) -> Option<&FetchArtifact> {
        self.entries.get(cache_key)
    }

    /// Store an artifact and return its cache key.
    pub fn put(&mut self, artifact: FetchArtifact) -> String {
        let key = artifact.cache_key.clone();
        self.entries.insert(key.clone(), artifact);
        key
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

