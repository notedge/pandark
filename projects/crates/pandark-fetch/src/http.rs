use std::collections::BTreeMap;
use std::io::Read;
use std::time::Duration;

use pandark_types::{BodyStorage, RedirectHop, Result, TransportProvenance};
use super::{FetchRequest, Transport, build_artifact, map_transport_error};

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

        let agent = ureq::AgentBuilder::new()
            .timeout(self.timeout)
            .redirects(0)
            .build();

        let mut current_url = request.url.clone();
        let mut redirect_chain = Vec::new();

        for _ in 0..=self.max_redirects {
            let mut http_request = agent.get(current_url.as_str());
            for (key, value) in &request.headers {
                http_request = http_request.set(key, value);
            }

            let response = http_request
                .call()
                .map_err(|error| map_transport_error(error.to_string()))?;
            let status = response.status();
            if is_redirect(status) {
                let location = response
                    .header("location")
                    .ok_or_else(|| map_transport_error("redirect without location header"))?;
                redirect_chain.push(RedirectHop {
                    url: current_url.clone(),
                    status,
                });
                current_url = current_url
                    .join(location)
                    .map_err(|error| map_transport_error(error.to_string()))?;
                continue;
            }

            let etag = response.header("etag").map(str::to_string);
            let headers = response_headers(&response);
            let mut reader = response.into_reader();
            let mut body = Vec::new();
            let mut chunk = [0u8; 8192];
            while body.len() < self.max_body_bytes {
                let remaining = self.max_body_bytes - body.len();
                let to_read = remaining.min(chunk.len());
                let read = reader
                    .read(&mut chunk[..to_read])
                    .map_err(|error| map_transport_error(error.to_string()))?;
                if read == 0 {
                    break;
                }
                body.extend_from_slice(&chunk[..read]);
            }
            if body.len() >= self.max_body_bytes {
                let extra = reader
                    .read(&mut [0u8; 1])
                    .map_err(|error| map_transport_error(error.to_string()))?;
                if extra > 0 {
                    return Err(map_transport_error(format!(
                        "response body exceeds budget of {} bytes",
                        self.max_body_bytes
                    )));
                }
            }

            return Ok(build_artifact(
                request.url.clone(),
                current_url,
                status,
                headers,
                BodyStorage::inline(body),
                redirect_chain,
                TransportProvenance::Network,
                etag.as_deref(),
            ));
        }

        Err(map_transport_error(format!(
            "redirect chain exceeded {} hops for `{}`",
            self.max_redirects,
            request.url
        )))
    }
}

fn is_redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

fn response_headers(response: &ureq::Response) -> BTreeMap<String, String> {
    let mut headers = BTreeMap::new();
    for name in response.headers_names() {
        if let Some(value) = response.header(&name) {
            headers.insert(name.to_ascii_lowercase(), value.to_string());
        }
    }
    headers
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::*;

    #[test]
    fn rejects_non_http_schemes() {
        let transport = HttpTransport::new();
        let request = FetchRequest {
            url: Url::parse("file:///tmp/page.html").expect("url"),
            headers: BTreeMap::new(),
        };
        let error = transport.fetch(&request).unwrap_err();
        assert!(error.to_string().contains("http transport"));
    }
}
