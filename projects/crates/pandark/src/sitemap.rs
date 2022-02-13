use std::collections::{BTreeMap, HashSet};
use std::io::Read;

use oak_xml::ast::XmlValue;
use pandark_fetch::{FetchClient, FetchRequest, Transport};
use pandark_types::{CrawlError, CrawlEvent, DiscoverySource, FrontierItem, Result};

use crate::crawl::CrawlState;

const MAX_SITEMAP_LOCATIONS: usize = 10_000;

fn locations(body: &[u8]) -> Result<(bool, Vec<String>)> {
    if body.len() > 1024 * 1024 {
        return Err(CrawlError::InvalidInput("sitemap exceeds one MiB XML budget".into()));
    }
    let text = std::str::from_utf8(body).map_err(|error| CrawlError::InvalidInput(error.to_string()))?;
    let document = oak_xml::parse(text).map_err(|error| CrawlError::InvalidInput(error.to_string()))?;
    let root = match &document {
        XmlValue::Element(root) => Some(root),
        XmlValue::Fragment(values) => values.iter().find_map(XmlValue::as_element),
        _ => None,
    }.ok_or_else(|| CrawlError::InvalidInput("sitemap has no XML root".into()))?;
    let (index, entry) = match root.name.rsplit(':').next() {
        Some("urlset") => (false, "url"),
        Some("sitemapindex") => (true, "sitemap"),
        _ => return Err(CrawlError::InvalidInput("unsupported sitemap root".into())),
    };
    let mut locations = Vec::new();
    for child in root.children.iter().filter_map(XmlValue::as_element) {
        if child.name.rsplit(':').next() != Some(entry) {
            continue;
        }
        for location in child.children.iter().filter_map(XmlValue::as_element) {
            if location.name.rsplit(':').next() != Some("loc") {
                continue;
            }
            let mut text = String::new();
            for value in &location.children {
                match value {
                    XmlValue::Text(value) | XmlValue::CData(value) => text.push_str(value),
                    _ => return Err(CrawlError::InvalidInput("sitemap loc must contain text only".into())),
                }
            }
            locations.push(text.trim().to_string());
            if locations.len() > MAX_SITEMAP_LOCATIONS {
                return Err(CrawlError::InvalidInput("sitemap location count exceeds budget".into()));
            }
        }
    }
    Ok((index, locations))
}

fn is_xml_content_type(artifact: &pandark_types::FetchArtifact) -> bool {
    let value = artifact
        .content_type
        .as_deref()
        .or_else(|| artifact.response_headers.get("content-type").map(String::as_str))
        .map(|value| value.split(';').next().unwrap_or(value).trim().to_ascii_lowercase());
    matches!(value.as_deref(), Some("application/xml" | "text/xml" | "application/xml+rss" | "application/xhtml+xml"))
}

fn sitemap_body(artifact: &pandark_types::FetchArtifact) -> Result<Vec<u8>> {
    let body = artifact.body_bytes().ok_or_else(|| CrawlError::InvalidInput("sitemap body unavailable".into()))?;
    let encoding = artifact.response_headers.get("content-encoding").map(|value| value.trim().to_ascii_lowercase());
    let decoded = match encoding.as_deref() {
        None | Some("") | Some("identity") => body.to_vec(),
        Some("gzip") | Some("x-gzip") => {
            let decoder = flate2::read::GzDecoder::new(body);
            let mut decoded = Vec::new();
            decoder.take(1024 * 1024 + 1).read_to_end(&mut decoded)
                .map_err(|error| CrawlError::InvalidInput(format!("sitemap gzip decode failed: {error}")))?;
            decoded
        }
        Some(other) => return Err(CrawlError::InvalidInput(format!("unsupported sitemap content encoding `{other}`"))),
    };
    if decoded.len() > 1024 * 1024 {
        return Err(CrawlError::InvalidInput("sitemap exceeds one MiB decoded XML budget".into()));
    }
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use pandark_fetch::build_artifact;
    use pandark_types::{BodyStorage, TransportProvenance};
    use url::Url;

    use super::{locations, sitemap_body, MAX_SITEMAP_LOCATIONS};

    #[test]
    fn rejects_unsupported_root_and_nested_location_payloads() {
        assert!(locations(b"<html><body>not a sitemap</body></html>").is_err());
        assert!(locations(b"<urlset><url><loc><span>https://example.com/</span></loc></url></urlset>").is_err());
        assert!(locations(&[0xff]).is_err());
    }

    #[test]
    fn rejects_xml_above_the_parse_budget() {
        assert!(locations(&vec![b' '; 1024 * 1024 + 1]).is_err());
    }

    #[test]
    fn rejects_excessive_location_count() {
        let mut xml = String::from("<urlset>");
        for index in 0..=MAX_SITEMAP_LOCATIONS {
            xml.push_str(&format!("<url><loc>https://example.com/{index}</loc></url>"));
        }
        xml.push_str("</urlset>");
        assert!(locations(xml.as_bytes()).is_err());
    }

    #[test]
    fn decodes_gzip_and_rejects_unknown_encoding() {
        let mut compressed = Vec::new();
        let mut encoder = flate2::write::GzEncoder::new(&mut compressed, flate2::Compression::fast());
        encoder.write_all(b"<urlset/>").expect("compress");
        encoder.finish().expect("finish");
        let url = Url::parse("https://example.com/map.xml").expect("url");
        let artifact = build_artifact(
            url.clone(), url.clone(), 200,
            std::collections::BTreeMap::from([
                ("content-type".into(), "application/xml".into()),
                ("content-encoding".into(), "gzip".into()),
            ]),
            BodyStorage::inline(compressed), Vec::new(), TransportProvenance::Test, None,
        );
        assert_eq!(sitemap_body(&artifact).expect("decode"), b"<urlset/>");

        let mut unknown = artifact;
        unknown.response_headers.insert("content-encoding".into(), "br".into());
        assert!(sitemap_body(&unknown).is_err());
    }
}

pub(crate) fn discover<T: Transport>(
    state: &mut CrawlState,
    source: &FrontierItem,
    client: &mut FetchClient<T>,
    seen: &mut HashSet<String>,
) {
    let mut pending = client.robots().sitemaps_for_url(&source.request_url).to_vec();
    while let Some(location) = pending.pop() {
        let Ok(url) = pandark_types::canonicalize_request_url(&location) else { continue };
        if url.origin() != source.request_url.origin() || !matches!(url.scheme(), "http" | "https") {
            state.report.events.push(CrawlEvent::Skipped { url, reason: "sitemap-origin-policy".into() });
            continue;
        }
        if !seen.insert(url.to_string()) {
            continue;
        }
        if state.requests_used >= state.request.budget.max_requests {
            state.report.events.push(CrawlEvent::Skipped { url, reason: "sitemap-request-budget-exhausted".into() });
            continue;
        }
        state.requests_used += 1;
        let fetched = client.fetch(&FetchRequest { url: url.clone(), headers: BTreeMap::new() });
        let parsed = fetched.and_then(|artifact| {
            state.report.events.push(CrawlEvent::Fetched {
                url: artifact.final_url.clone(), status: artifact.status,
                cache_hit: artifact.transport_provenance == pandark_types::TransportProvenance::CacheHit,
            });
            if artifact.status != 200 || artifact.final_url.origin() != source.request_url.origin() {
                return Err(CrawlError::InvalidInput("sitemap response status/origin rejected".into()));
            }
            if !is_xml_content_type(&artifact) {
                return Err(CrawlError::InvalidInput("sitemap response content type is not XML".into()));
            }
            let body = sitemap_body(&artifact)?;
            locations(&body)
        });
        let (index, entries) = match parsed {
            Ok(parsed) => parsed,
            Err(error) => {
                state.report.failed += 1;
                state.report.events.push(CrawlEvent::Failed { url, reason: format!("sitemap-discovery: {error}") });
                continue;
            }
        };
        if index {
            pending.extend(entries);
            continue;
        }
        for entry in entries {
            let Ok(url) = pandark_types::canonicalize_request_url(&entry) else { continue };
            if url.origin() != source.request_url.origin() || !matches!(url.scheme(), "http" | "https") {
                state.report.events.push(CrawlEvent::Skipped { url, reason: "sitemap-origin-policy".into() });
                continue;
            }
            let mut item = FrontierItem::seed(url.clone(), pandark_types::site_key(&url));
            item.discovery = DiscoverySource::Manifest;
            item.depth = source.depth;
            if state.queue.push(item.clone()) {
                state.report.events.push(CrawlEvent::Discovered { url, depth: item.depth });
            }
        }
    }
}
