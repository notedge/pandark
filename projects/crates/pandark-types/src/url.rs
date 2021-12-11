use std::time::{SystemTime, UNIX_EPOCH};

use url::Url;

use crate::error::{CrawlError, Result};

/// Canonicalize a request URL for frontier deduplication.
///
/// Rules are intentionally conservative: scheme and host are lowercased,
/// default ports removed, path segments normalized, and trailing slashes on
/// directory-like paths preserved. Query strings are kept intact because they
/// may affect resource identity.
pub fn canonicalize_request_url(raw: &str) -> Result<Url> {
    let parsed = Url::parse(raw).map_err(|error| CrawlError::InvalidInput(error.to_string()))?;
    canonicalize_parsed_url(parsed)
}

/// Canonicalize an already parsed URL.
pub fn canonicalize_parsed_url(mut url: Url) -> Result<Url> {
    if url.scheme() != "http" && url.scheme() != "https" && url.scheme() != "file" {
        return Err(CrawlError::InvalidInput(format!(
            "unsupported URL scheme `{}`",
            url.scheme()
        )));
    }

    let host = url
        .host_str()
        .ok_or_else(|| CrawlError::InvalidInput("URL must include a host".into()))?
        .to_ascii_lowercase();
    url.set_host(Some(&host))
        .map_err(|error| CrawlError::InvalidInput(error.to_string()))?;

    if (url.scheme() == "http" && url.port() == Some(80))
        || (url.scheme() == "https" && url.port() == Some(443))
    {
        let _ = url.set_port(None);
    }

    let mut path = url.path().to_string();
    if path.is_empty() {
        path.push('/');
    }
    while path.contains("//") {
        path = path.replace("//", "/");
    }
    if path.len() > 1 && path.ends_with('/') && !path.ends_with("/./") {
        // keep trailing slash for directory-like paths
    }
    url.set_path(&path);

    Ok(url)
}

/// Site key used for politeness partitioning.
pub fn site_key(url: &Url) -> String {
    let host = url.host_str().unwrap_or("unknown");
    match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    }
}

/// Current unix epoch seconds.
pub fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{canonicalize_request_url, site_key};

    #[test]
    fn lowercases_host_and_keeps_query() {
        let url = canonicalize_request_url("HTTPS://Example.COM/docs?id=1").expect("url");
        assert_eq!(url.as_str(), "https://example.com/docs?id=1");
    }

    #[test]
    fn site_key_includes_non_default_port() {
        let url = canonicalize_request_url("http://example.com:8080/a").expect("url");
        assert_eq!(site_key(&url), "example.com:8080");
    }
}
