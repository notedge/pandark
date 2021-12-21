use pandark_types::{CrawlError, Result};
use url::Url;

use super::{FetchArtifact, FetchRequest, FileTransport, HttpTransport, Transport};

/// Built-in transports selected from a seed URL scheme.
#[derive(Debug, Clone)]
pub enum SeedTransport {
    /// Local `file://` pages and fixtures.
    File(FileTransport),
    /// Remote `http://` and `https://` pages.
    Http(HttpTransport),
}

impl Transport for SeedTransport {
    fn fetch(&self, request: &FetchRequest) -> Result<FetchArtifact> {
        match self {
            Self::File(transport) => transport.fetch(request),
            Self::Http(transport) => transport.fetch(request),
        }
    }
}

/// Pick the default transport for a parsed seed URL.
pub fn transport_for_url(url: &Url) -> Result<SeedTransport> {
    match url.scheme() {
        "file" => Ok(SeedTransport::File(FileTransport::new())),
        "http" | "https" => Ok(SeedTransport::Http(HttpTransport::new())),
        other => Err(CrawlError::InvalidInput(format!(
            "unsupported seed scheme `{other}` for `{}`",
            url
        ))),
    }
}
