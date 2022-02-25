//! Extracted from `src/client.rs`.

use std::time::{Duration, Instant};

use pandark_fetch::{FetchClient, MemoryEntry, MemoryTransport, RobotsPolicy, fetch_url, site_key_for_url};
use pandark_types::{CrawlError, PolitenessProfile, TransportProvenance};
use url::Url;

#[test]
fn cache_hit_marks_provenance() {
    let mut transport = MemoryTransport::new();
    transport.insert("https://example.com/", MemoryEntry { status: 200, headers: Default::default(), body: b"hello".to_vec() });
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
    let mut client = FetchClient::new(transport, RobotsPolicy::from_profile(PolitenessProfile::Conservative));
    let url = Url::parse("https://example.com/admin/page").expect("url");
    let error = fetch_url(&mut client, url).expect_err("denied");
    assert!(matches!(error, CrawlError::InvalidInput(_)));
}

#[test]
fn loads_robots_txt_before_fetching_page() {
    let mut transport = MemoryTransport::new();
    transport.insert(
        "https://example.com/robots.txt",
        MemoryEntry { status: 200, headers: Default::default(), body: b"User-agent: *\nDisallow: /secret\n".to_vec() },
    );
    transport
        .insert("https://example.com/open", MemoryEntry { status: 200, headers: Default::default(), body: b"open".to_vec() });
    let mut client = FetchClient::new(transport, RobotsPolicy::from_profile(PolitenessProfile::Developer));
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
        MemoryEntry { status: 200, headers: Default::default(), body: b"private".to_vec() },
    );
    let mut client = FetchClient::new(transport, RobotsPolicy::from_profile(PolitenessProfile::Conservative));
    let url = Url::parse("https://example.com/private/data").expect("url");
    let error = fetch_url(&mut client, url).expect_err("fallback deny");
    assert!(matches!(error, CrawlError::InvalidInput(_)));
}

#[test]
fn applies_loaded_crawl_delay_between_same_site_requests() {
    let mut transport = MemoryTransport::new();
    transport.insert(
        "https://example.com/robots.txt",
        MemoryEntry { status: 200, headers: Default::default(), body: b"User-agent: *\nCrawl-delay: 1\n".to_vec() },
    );
    for path in ["/one", "/two"] {
        transport.insert(
            format!("https://example.com{path}"),
            MemoryEntry { status: 200, headers: Default::default(), body: path.as_bytes().to_vec() },
        );
    }
    let mut client = FetchClient::new(transport, RobotsPolicy::from_profile(PolitenessProfile::Developer));
    fetch_url(&mut client, Url::parse("https://example.com/one").expect("url")).expect("first page");
    let started = Instant::now();
    fetch_url(&mut client, Url::parse("https://example.com/two").expect("url")).expect("second page");
    assert!(started.elapsed() >= Duration::from_millis(900));
}

#[test]
fn failed_transport_attempt_keeps_site_timing_state() {
    let mut policy = RobotsPolicy::from_profile(PolitenessProfile::Developer);
    let url = Url::parse("https://example.com/missing").expect("url");
    let site = site_key_for_url(&url);
    policy.load_site_rules(&site, "User-agent: *\nCrawl-delay: 1\n");
    let mut client = FetchClient::new(MemoryTransport::new(), policy);
    fetch_url(&mut client, url).expect_err("missing fixture");
    assert!(client.has_tracked_site(&site));
}
