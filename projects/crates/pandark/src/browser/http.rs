use pandark_types::{BrowserSnapshot, CrawlError, Result};
use boost_http::{HttpTransport, HttpTransportConfig, Method, Request, Transport};
use url::Url;

/// Remote browser worker that returns `BrowserSnapshot` JSON over HTTP.
#[derive(Debug, Clone)]
pub struct HttpBrowserProvider {
    /// Worker base URL, for example `http://127.0.0.1:9333/`.
    endpoint: Url,
    /// Profile label recorded in captured snapshots.
    profile_id: String,
}

impl HttpBrowserProvider {
    /// Build a provider for the given worker base URL.
    pub fn new(endpoint: Url) -> Self {
        Self {
            endpoint,
            profile_id: "http-browser".into(),
        }
    }

    /// Override the profile label stored on fetched snapshots.
    pub fn with_profile_id(mut self, profile_id: impl Into<String>) -> Self {
        self.profile_id = profile_id.into();
        self
    }

    /// Build the worker snapshot URL for a page.
    pub fn snapshot_url(&self, page_url: &Url) -> Url {
        let mut endpoint = self.endpoint.clone();
        endpoint
            .query_pairs_mut()
            .append_pair("url", page_url.as_str());
        endpoint
    }
}

impl super::provider::BrowserProvider for HttpBrowserProvider {
    fn capture_snapshot(&self, url: &Url) -> Result<BrowserSnapshot> {
        let request_url = self.snapshot_url(url);
        let transport = HttpTransport::with_config(HttpTransportConfig {
            max_body_bytes: 8 * 1024 * 1024,
            max_redirects: 4,
            ..HttpTransportConfig::default()
        });
        let request = Request::new(Method::Get, request_url);
        let response = transport
            .send(&request)
            .map_err(|error| CrawlError::InvalidInput(format!("browser worker request failed: {error}")))?;
        let status = response.status;
        if !(200..300).contains(&status) {
            return Err(CrawlError::InvalidInput(format!(
                "browser worker returned status {status}"
            )));
        }
        let snapshot: BrowserSnapshot = serde_json::from_slice(&response.body).map_err(|error| {
            CrawlError::InvalidInput(format!("browser worker JSON invalid: {error}"))
        })?;
        Ok(snapshot)
    }
}

