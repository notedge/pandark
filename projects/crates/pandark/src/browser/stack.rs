use pandark_types::{BrowserSnapshot, CrawlError, Result};
use url::Url;

use super::http::HttpBrowserProvider;
use super::provider::{BrowserProvider, FixtureBrowserProvider};
use super::snapshot_io::{load_fixture_provider_from_dirs, parse_browser_dirs};

/// Ordered browser providers used during crawl fallback.
#[derive(Debug, Default)]
pub struct BrowserProviderStack {
    fixture: Option<FixtureBrowserProvider>,
    http: Option<HttpBrowserProvider>,
}

impl BrowserProviderStack {
    /// Empty stack with no providers configured.
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach a fixture directory provider.
    pub fn with_fixture(mut self, fixture: FixtureBrowserProvider) -> Self {
        self.fixture = Some(fixture);
        self
    }

    /// Attach an HTTP worker provider.
    pub fn with_http(mut self, http: HttpBrowserProvider) -> Self {
        self.http = Some(http);
        self
    }

    /// Whether any provider is configured.
    pub fn is_empty(&self) -> bool {
        self.fixture.is_none() && self.http.is_none()
    }
}

/// Build a browser provider stack from CLI-style configuration strings.
pub fn build_browser_stack(
    fixture_dirs: Option<&str>,
    http_endpoint: Option<&str>,
) -> Result<BrowserProviderStack> {
    let mut stack = BrowserProviderStack::new();
    if let Some(spec) = fixture_dirs {
        let dirs = parse_browser_dirs(spec);
        if !dirs.is_empty() {
            stack = stack.with_fixture(load_fixture_provider_from_dirs(&dirs)?);
        }
    }
    if let Some(endpoint) = http_endpoint {
        let url = Url::parse(endpoint).map_err(|error| {
            CrawlError::InvalidInput(format!("invalid browser endpoint `{endpoint}`: {error}"))
        })?;
        stack = stack.with_http(HttpBrowserProvider::new(url));
    }
    Ok(stack)
}

impl BrowserProvider for BrowserProviderStack {
    fn capture_snapshot(&self, url: &Url) -> Result<BrowserSnapshot> {
        let mut last_error = None;
        if let Some(fixture) = &self.fixture {
            match fixture.capture_snapshot(url) {
                Ok(snapshot) => return Ok(snapshot),
                Err(error) => last_error = Some(error),
            }
        }
        if let Some(http) = &self.http {
            return http.capture_snapshot(url);
        }
        Err(last_error.unwrap_or_else(|| {
            CrawlError::InvalidInput(format!("no browser snapshot available for `{url}`"))
        }))
    }
}

#[cfg(test)]
mod tests {
    use pandark_types::{BrowserSnapshot, PageChallengeState};

    use super::*;

    #[test]
    fn fixture_provider_wins_before_http_worker() {
        let mut fixture = FixtureBrowserProvider::new();
        fixture.insert(BrowserSnapshot {
            requested_url: Url::parse("https://example.com/page").expect("url"),
            final_url: Url::parse("https://example.com/page").expect("url"),
            document_html: "<html><body>fixture</body></html>".into(),
            captured_at_epoch: 1,
            browser_engine: "fixture".into(),
            profile_id: "test".into(),
            challenge_state: PageChallengeState::Normal,
        });
        let http = HttpBrowserProvider::new(
            Url::parse("http://127.0.0.1:1/unreachable").expect("url"),
        );
        let stack = BrowserProviderStack::new()
            .with_fixture(fixture)
            .with_http(http);
        let snapshot = stack
            .capture_snapshot(&Url::parse("https://example.com/page").expect("url"))
            .expect("snapshot");
        assert_eq!(snapshot.browser_engine, "fixture");
    }
}
