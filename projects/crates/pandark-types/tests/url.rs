//! Extracted from `src/url.rs`.

use pandark_types::{canonicalize_request_url, site_key};
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
