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

#[cfg(test)]
mod tests {
    use url::Url;

    use pandark_types::{BodyStorage, TransportProvenance, now_epoch};

    use super::*;

    #[test]
    fn stores_and_retrieves_by_cache_key() {
        let mut cache = ResponseCache::new();
        let artifact = FetchArtifact {
            request_url: Url::parse("https://example.com/").expect("url"),
            final_url: Url::parse("https://example.com/").expect("url"),
            status: 200,
            response_headers: Default::default(),
            content_type: Some("text/html".into()),
            body: BodyStorage::inline(b"ok"),
            redirect_chain: Vec::new(),
            retrieved_at_epoch: now_epoch(),
            transport_provenance: TransportProvenance::Test,
            cache_key: "https://example.com/".into(),
        };
        cache.put(artifact);
        assert!(cache.get("https://example.com/").is_some());
    }
}
