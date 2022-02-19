use std::{
    collections::BTreeMap,
    thread,
    time::{Duration, Instant},
};
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
    last_site_requests: BTreeMap<String, Instant>,
}

impl<T: Transport> FetchClient<T> {
    /// Build a client around a transport and robots policy.
    pub fn new(transport: T, robots: RobotsPolicy) -> Self {
        Self {
            transport,
            cache: LayeredResponseCache::memory_only(),
            robots,
            last_site_requests: BTreeMap::new(),
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

    /// Whether a site key was recorded after a fetch attempt.
    pub fn has_tracked_site(&self, site: &str) -> bool {
        self.last_site_requests.contains_key(site)
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

        self.wait_for_crawl_delay(&request.url);
        self.last_site_requests
            .insert(site_key_for_url(&request.url), Instant::now());
        let artifact = self.transport.fetch(request)?;
        let key = self.cache.put(artifact)?;
        Ok(self
            .cache
            .get(&key)?
            .expect("artifact inserted into cache"))
    }

    fn wait_for_crawl_delay(&self, url: &Url) {
        let Some(delay_seconds) = self.robots.crawl_delay_for_url(url) else {
            return;
        };
        let Some(last_request) = self.last_site_requests.get(&site_key_for_url(url)) else {
            return;
        };
        let delay = Duration::from_secs(u64::from(delay_seconds));
        if let Some(remaining) = delay.checked_sub(last_request.elapsed()) {
            thread::sleep(remaining);
        }
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

