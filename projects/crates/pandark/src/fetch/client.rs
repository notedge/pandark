use std::collections::BTreeMap;
use std::path::Path;

use pandark_types::{CrawlError, FetchArtifact, Result, TransportProvenance};
use url::Url;

use super::cache::ResponseCache;
use super::disk_cache::{DiskResponseCache, LayeredResponseCache};
use super::robots::RobotsPolicy;
use super::robots_txt::site_key_for_url;
use super::{FetchRequest, Transport};

/// Fetch orchestration with robots checks and response caching.
pub struct FetchClient<T: Transport> {
    transport: T,
    cache: LayeredResponseCache,
    robots: RobotsPolicy,
}

impl<T: Transport> FetchClient<T> {
    /// Build a client around a transport and robots policy.
    pub fn new(transport: T, robots: RobotsPolicy) -> Self {
        Self {
            transport,
            cache: LayeredResponseCache::memory_only(),
            robots,
        }
    }

    /// Persist responses under the given cache directory.
    pub fn with_disk_cache_dir(mut self, dir: impl AsRef<Path>) -> Result<Self> {
        self.cache = LayeredResponseCache::with_disk(DiskResponseCache::open(dir)?);
        Ok(self)
    }

    /// Access the in-memory response cache.
    pub fn cache(&self) -> &ResponseCache {
        self.cache.memory()
    }

    /// Mutable in-memory cache access for tests.
    pub fn cache_mut(&mut self) -> &mut ResponseCache {
        self.cache.memory_mut()
    }

    /// Robots policy used for admission checks.
    pub fn robots(&self) -> &RobotsPolicy {
        &self.robots
    }

    /// Fetch one URL through robots, cache, and transport.
    pub fn fetch(&mut self, request: &FetchRequest) -> Result<FetchArtifact> {
        if should_resolve_robots(&request.url) {
            self.ensure_site_robots(request)?;
        }

        if !self.robots.is_allowed(&request.url) {
            return Err(CrawlError::InvalidInput(format!(
                "robots policy denied `{}`",
                request.url
            )));
        }

        let request_key = request.url.to_string();
        if let Some(cached) = self.cache.get_by_request_url(&request_key)? {
            return Ok(with_cache_provenance(cached));
        }

        let cache_key = super::cache_key_for(&request.url, None);
        if let Some(cached) = self.cache.get(&cache_key)? {
            return Ok(with_cache_provenance(cached));
        }

        let artifact = self.transport.fetch(request)?;
        let key = self.cache.put(artifact)?;
        Ok(self
            .cache
            .get(&key)?
            .expect("artifact inserted into cache"))
    }

    fn ensure_site_robots(&mut self, request: &FetchRequest) -> Result<()> {
        if self.robots.has_site_rules(&request.url) {
            return Ok(());
        }

        let site_key = site_key_for_url(&request.url);
        let robots_url = robots_txt_url_for(&request.url)?;
        let robots_request = FetchRequest {
            url: robots_url,
            headers: BTreeMap::new(),
        };

        match self.transport.fetch(&robots_request) {
            Ok(artifact) if artifact.status == 200 => {
                if let Some(body) = artifact.body_bytes() {
                    let text = String::from_utf8_lossy(body);
                    self.robots.load_site_rules(&site_key, &text);
                } else {
                    self.robots.mark_site_unavailable(&site_key);
                }
            }
            Ok(_) => self.robots.mark_site_missing(&site_key),
            Err(_) => self.robots.mark_site_unavailable(&site_key),
        }
        Ok(())
    }
}

fn should_resolve_robots(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https") && !is_robots_txt_url(url)
}

fn is_robots_txt_url(url: &Url) -> bool {
    url.path().eq_ignore_ascii_case("/robots.txt")
}

fn robots_txt_url_for(base: &Url) -> Result<Url> {
    let mut url = base.clone();
    url.set_path("/robots.txt");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

fn with_cache_provenance(mut artifact: FetchArtifact) -> FetchArtifact {
    artifact.transport_provenance = TransportProvenance::CacheHit;
    artifact
}

/// Convenience fetch with empty headers.
pub fn fetch_url<T: Transport>(
    client: &mut FetchClient<T>,
    url: Url,
) -> Result<FetchArtifact> {
    client.fetch(&FetchRequest {
        url,
        headers: BTreeMap::new(),
    })
}

#[cfg(test)]
mod tests {
    use pandark_types::{CrawlError, PolitenessProfile, TransportProvenance};
    use url::Url;

    use super::*;
    use crate::fetch::memory::{MemoryEntry, MemoryTransport};

    #[test]
    fn cache_hit_marks_provenance() {
        let mut transport = MemoryTransport::new();
        transport.insert(
            "https://example.com/",
            MemoryEntry {
                status: 200,
                headers: Default::default(),
                body: b"hello".to_vec(),
            },
        );
        let mut client = FetchClient::new(transport, RobotsPolicy::from_profile(PolitenessProfile::Developer));
        let url = Url::parse("https://example.com/").expect("url");
        let first = fetch_url(&mut client, url.clone()).expect("first");
        assert_eq!(first.transport_provenance, TransportProvenance::Test);
        let second = fetch_url(&mut client, url).expect("second");
        assert_eq!(second.transport_provenance, TransportProvenance::CacheHit);
    }

    #[test]
    fn robots_denies_admin_path() {
        let transport = MemoryTransport::new();
        let mut client = FetchClient::new(
            transport,
            RobotsPolicy::from_profile(PolitenessProfile::Conservative),
        );
        let url = Url::parse("https://example.com/admin/page").expect("url");
        let error = fetch_url(&mut client, url).expect_err("denied");
        assert!(matches!(error, CrawlError::InvalidInput(_)));
    }

    #[test]
    fn loads_robots_txt_before_fetching_page() {
        let mut transport = MemoryTransport::new();
        transport.insert(
            "https://example.com/robots.txt",
            MemoryEntry {
                status: 200,
                headers: Default::default(),
                body: b"User-agent: *\nDisallow: /secret\n".to_vec(),
            },
        );
        transport.insert(
            "https://example.com/open",
            MemoryEntry {
                status: 200,
                headers: Default::default(),
                body: b"open".to_vec(),
            },
        );
        let mut client = FetchClient::new(
            transport,
            RobotsPolicy::from_profile(PolitenessProfile::Developer),
        );
        let allowed = Url::parse("https://example.com/open").expect("url");
        fetch_url(&mut client, allowed.clone()).expect("allowed page");
        let denied = Url::parse("https://example.com/secret/page").expect("url");
        let error = fetch_url(&mut client, denied).expect_err("denied by robots.txt");
        assert!(matches!(error, CrawlError::InvalidInput(_)));
        assert!(client.robots().has_site_rules(&allowed));
    }

    #[test]
    fn missing_robots_txt_uses_fallback_paths() {
        let mut transport = MemoryTransport::new();
        transport.insert(
            "https://example.com/private/data",
            MemoryEntry {
                status: 200,
                headers: Default::default(),
                body: b"private".to_vec(),
            },
        );
        let mut client = FetchClient::new(
            transport,
            RobotsPolicy::from_profile(PolitenessProfile::Conservative),
        );
        let url = Url::parse("https://example.com/private/data").expect("url");
        let error = fetch_url(&mut client, url).expect_err("fallback deny");
        assert!(matches!(error, CrawlError::InvalidInput(_)));
    }
}
