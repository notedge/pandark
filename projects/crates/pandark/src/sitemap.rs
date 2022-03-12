use std::{
    collections::{BTreeMap, HashSet},
};

use acorn_codec::{CodecLimits, compression::gzip::Gzip};
use boost_diagnostic::{DiagnosticSeverity, Message, MessageArg};
use oak_xml::ast::XmlValue;
use pandark_fetch::{FetchClient, FetchRequest, Transport};
use pandark_types::{CrawlError, CrawlEvent, DiscoverySource, FrontierItem, Result, diagnostics as pd};

use crate::crawl::CrawlState;

/// Maximum sitemap body budget before gzip decoding.
const SITEMAP_BUDGET_BYTES: u64 = 1024 * 1024;

/// Creates codec limits tuned for sitemap decoding.
fn sitemap_limits() -> CodecLimits {
    CodecLimits::new().with_max_output_bytes(SITEMAP_BUDGET_BYTES + 1)
}

/// Maximum number of sitemap locations accepted per document.
pub const MAX_SITEMAP_LOCATIONS: usize = 10_000;

/// Parse sitemap XML and return whether the root is an index plus location URLs.
pub fn locations(body: &[u8]) -> Result<(bool, Vec<String>)> {
    if body.len() > 1024 * 1024 {
        return Err(CrawlError::InvalidInput("sitemap exceeds one MiB XML budget".into()));
    }
    let text = std::str::from_utf8(body).map_err(|error| {
        let diagnostic = boost_diagnostic::Diagnostic::new(
            pd::code(pd::EXTRACT_PARSE_FAILED),
            DiagnosticSeverity::Error,
            pd::origin_extract(),
            Message::new("pandark.diagnostic.sitemapUtf8")
                .with_arg("error", MessageArg::Text(error.to_string()))
                .with_fallback(format!("sitemap UTF-8 decode failed: {error}")),
        );
        CrawlError::from(diagnostic)
    })?;
    let document = oak_xml::parse(text).map_err(|error| {
        let diagnostic = boost_diagnostic::Diagnostic::new(
            pd::code(pd::EXTRACT_PARSE_FAILED),
            DiagnosticSeverity::Error,
            pd::origin_extract(),
            Message::new("pandark.diagnostic.sitemapXmlParse")
                .with_arg("error", MessageArg::Text(error.to_string()))
                .with_fallback(format!("sitemap XML parse failed: {error}")),
        );
        CrawlError::from(diagnostic)
    })?;
    let root = match &document {
        XmlValue::Element(root) => Some(root),
        XmlValue::Fragment(values) => values.iter().find_map(XmlValue::as_element),
        _ => None,
    }
    .ok_or_else(|| CrawlError::InvalidInput("sitemap has no XML root".into()))?;
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

/// Decode a fetched sitemap artifact body, honoring `content-encoding`.
pub fn sitemap_body(artifact: &pandark_types::FetchArtifact) -> Result<Vec<u8>> {
    let body = artifact.body_bytes().ok_or_else(|| CrawlError::InvalidInput("sitemap body unavailable".into()))?;
    let encoding = artifact.response_headers.get("content-encoding").map(|value| value.trim().to_ascii_lowercase());
    let decoded = match encoding.as_deref() {
        None | Some("") | Some("identity") => body.to_vec(),
        Some("gzip") | Some("x-gzip") => {
            let facts = Gzip::decode(body, sitemap_limits()).map_err(|error| {
                let diagnostic = boost_diagnostic::Diagnostic::new(
                    pd::code(pd::FETCH_DECODE_FAILED),
                    DiagnosticSeverity::Error,
                    pd::origin_fetch(),
                    Message::new("pandark.diagnostic.sitemapGzipDecode")
                        .with_arg("error", MessageArg::Text(error.to_string()))
                        .with_fallback(format!("sitemap gzip decode failed: {error}")),
                );
                CrawlError::from(diagnostic)
            })?;
            facts.value
        }
        Some(other) => return Err(CrawlError::InvalidInput(format!("unsupported sitemap content encoding `{other}`"))),
    };
    if decoded.len() > SITEMAP_BUDGET_BYTES as usize {
        let diagnostic = boost_diagnostic::Diagnostic::new(
            pd::code(pd::FETCH_BODY_BUDGET),
            DiagnosticSeverity::Error,
            pd::origin_fetch(),
            Message::new("pandark.diagnostic.sitemapBodyBudget")
                .with_arg("budget_bytes", MessageArg::U64(SITEMAP_BUDGET_BYTES))
                .with_arg("actual_bytes", MessageArg::U64(decoded.len() as u64))
                .with_fallback(format!(
                    "sitemap decoded body {} exceeds one MiB budget",
                    decoded.len()
                )),
        );
        return Err(CrawlError::from(diagnostic));
    }
    Ok(decoded)
}

pub(crate) fn discover<T: Transport>(
    state: &mut CrawlState,
    source: &FrontierItem,
    client: &mut FetchClient<T>,
    seen: &mut HashSet<String>,
) {
    let mut pending = client.robots().sitemaps_for_url(&source.request_url).to_vec();
    while let Some(location) = pending.pop() {
        let Ok(url) = pandark_types::canonicalize_request_url(&location)
        else {
            continue;
        };
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
                url: artifact.final_url.clone(),
                status: artifact.status,
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
            let Ok(url) = pandark_types::canonicalize_request_url(&entry)
            else {
                continue;
            };
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
