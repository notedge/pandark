use std::collections::BTreeMap;
use std::time::Duration;

use boost_http::{
    Headers, HttpTransport as RequestHttpTransport, HttpTransportConfig, Method, Request,
    Transport as RequestTransport,
};
use pandark_types::{BodyStorage, RedirectHop, Result, TransportProvenance};

use super::{FetchRequest, Transport, build_artifact, map_transport_error};

fn headers_to_map(headers: &Headers) -> BTreeMap<String, String> {
    headers
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

/// Network transport for `http://` and `https://` URLs.
#[derive(Debug, Clone)]
pub struct HttpTransport {
    /// Maximum response body bytes to read.
    pub max_body_bytes: usize,
    /// Maximum redirect hops to follow.
    pub max_redirects: usize,
    /// Per-request timeout.
    pub timeout: Duration,
}

impl Default for HttpTransport {
    fn default() -> Self {
        Self {
            max_body_bytes: 8 * 1024 * 1024,
            max_redirects: 8,
            timeout: Duration::from_secs(30),
        }
    }
}

impl HttpTransport {
    /// Conservative defaults aligned with crawl budgets.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Transport for HttpTransport {
    fn fetch(&self, request: &FetchRequest) -> Result<pandark_types::FetchArtifact> {
        let scheme = request.url.scheme();
        if scheme != "http" && scheme != "https" {
            return Err(map_transport_error(format!(
                "http transport cannot fetch `{}`",
                request.url
            )));
        }

        let max_redirects = u32::try_from(self.max_redirects)
            .map_err(|_| map_transport_error("redirect budget exceeds supported range"))?;
        let transport = RequestHttpTransport::with_config(HttpTransportConfig {
            max_body_bytes: self.max_body_bytes,
            max_redirects,
            timeout: self.timeout,
            ..HttpTransportConfig::default()
        });
        let mut http_request = Request::new(Method::Get, request.url.clone());
        for (name, value) in &request.headers {
            http_request.set_header(name, value);
        }
        let response = transport.send(&http_request)
            .map_err(|error| map_transport_error(error.to_string()))?;
        let headers = headers_to_map(&response.headers);
        let etag = headers.get("etag").cloned();
        let redirect_chain = response.redirects.into_iter()
            .map(|redirect| RedirectHop { url: redirect.url, status: redirect.status })
            .collect();
        Ok(build_artifact(
            request.url.clone(),
            response.url,
            response.status,
            headers,
            BodyStorage::inline(response.body),
            redirect_chain,
            TransportProvenance::Network,
            etag.as_deref(),
        ))
    }
}

