use std::collections::BTreeMap;

use pandark_types::{BrowserSnapshot, CrawlError, Result};
use url::Url;

/// Conceptual browser provider capability. Implementations live outside `pandark` core transport.
pub trait BrowserProvider {
    /// Capture or retrieve a browser snapshot for the given URL.
    fn capture_snapshot(&self, url: &Url) -> Result<BrowserSnapshot>;
}

/// In-memory browser provider for tests and offline fixtures.
#[derive(Debug, Clone, Default)]
pub struct FixtureBrowserProvider {
    snapshots: BTreeMap<String, BrowserSnapshot>,
}

impl FixtureBrowserProvider {
    /// Empty fixture provider.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a snapshot keyed by final URL string.
    pub fn insert(&mut self, snapshot: BrowserSnapshot) {
        self.snapshots
            .insert(snapshot.final_url.to_string(), snapshot);
    }
}

impl BrowserProvider for FixtureBrowserProvider {
    fn capture_snapshot(&self, url: &Url) -> Result<BrowserSnapshot> {
        self.snapshots
            .get(url.as_str())
            .cloned()
            .ok_or_else(|| CrawlError::InvalidInput(format!("no fixture snapshot for `{url}`")))
    }
}
