use std::io::Read;

use pandark_types::{BrowserSnapshot, CrawlError, Result};
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

    fn snapshot_url(&self, page_url: &Url) -> Url {
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
        let response = ureq::get(request_url.as_str())
            .call()
            .map_err(|error| CrawlError::InvalidInput(format!("browser worker request failed: {error}")))?;
        let status = response.status();
        if !(200..300).contains(&status) {
            return Err(CrawlError::InvalidInput(format!(
                "browser worker returned status {status}"
            )));
        }
        let mut body = Vec::new();
        response
            .into_reader()
            .read_to_end(&mut body)
            .map_err(|error| CrawlError::InvalidInput(format!("browser worker body read failed: {error}")))?;
        let snapshot: BrowserSnapshot = serde_json::from_slice(&body).map_err(|error| {
            CrawlError::InvalidInput(format!("browser worker JSON invalid: {error}"))
        })?;
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_url_appends_page_query() {
        let provider = HttpBrowserProvider::new(
            Url::parse("http://127.0.0.1:9333/snapshot").expect("url"),
        );
        let page = Url::parse("https://example.com/private").expect("url");
        let request = provider.snapshot_url(&page);
        assert_eq!(
            request.as_str(),
            "http://127.0.0.1:9333/snapshot?url=https%3A%2F%2Fexample.com%2Fprivate"
        );
    }
}
