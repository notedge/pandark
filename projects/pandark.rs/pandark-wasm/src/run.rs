//! Capability-driven `fetch -> extract -> report` core.
//!
//! The core is target independent: it runs unchanged on the host during the
//! gate and inside `wasm32-unknown-unknown` at runtime.

use crawler_extract::{Extractor, HtmlExtractor, run_extract};
use crawler_types::{
    BodyStorage, CrawlBudgetStatus, CrawlError, CrawlReport, CrawlRequest, CrawlScope, ExtractBudget, ExtractContext,
    ExtractStatus, FetchArtifact, RedirectHop,
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::capability::{Capabilities, CapabilityError};

/// One run request issued by the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRequest {
    /// Seed URLs in crawl order.
    pub seeds: Vec<String>,
    /// Hard request limit for the run.
    pub max_requests: u32,
}

impl RunRequest {
    /// Build a request from seed URLs and a request limit.
    pub fn new(seeds: impl IntoIterator<Item = impl Into<String>>, max_requests: u32) -> Self {
        Self { seeds: seeds.into_iter().map(Into::into).collect(), max_requests }
    }
}

/// Kind of body the store received for one page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredPageKind {
    /// Extraction produced a semantic document graph.
    Document,
    /// Fetch succeeded but extraction produced no document.
    FetchOnly,
}

/// Per-page outcome surfaced in the report envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageOutcome {
    /// Requested URL.
    pub url: String,
    /// Final URL after host redirects.
    pub final_url: String,
    /// Extraction status.
    pub status: ExtractStatus,
    /// Store key used for the page payload.
    pub store_key: Option<String>,
    /// What the store received.
    pub stored: Option<StoredPageKind>,
}

/// Complete outcome of one capability-driven run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunOutcome {
    /// Versioned crawl report.
    pub report: CrawlReport,
    /// Serialized report envelope for JS consumers.
    pub report_json: String,
    /// Per-page outcomes in run order.
    pub pages: Vec<PageOutcome>,
}

/// Run `fetch -> extract -> report` with host-injected capabilities.
pub fn run_with_capabilities(request: &RunRequest, capabilities: &mut Capabilities<'_>) -> Result<RunOutcome, CapabilityError> {
    let crawl_request = CrawlRequest::from_seeds(request.seeds.iter().map(String::as_str)).map_err(crawl_error)?;
    let max_requests = request.max_requests.min(crawl_request.budget.max_requests);
    let scope = crawl_request.scope.clone().unwrap_or_else(CrawlScope::default);
    let mut report = CrawlReport::empty(crawl_request.seeds[0].url.clone());
    report.budget_status = CrawlBudgetStatus::Completed;
    let mut pages = Vec::new();
    let mut processed = 0u32;

    for seed in &crawl_request.seeds {
        if processed >= max_requests {
            report.budget_status = CrawlBudgetStatus::Exhausted;
            report.remaining_frontier = (crawl_request.seeds.len() as u32).saturating_sub(processed);
            break;
        }
        if capabilities.cancel.is_cancelled() {
            report.budget_status = CrawlBudgetStatus::Cancelled;
            break;
        }
        if !scope.allows(&seed.url) {
            report.skipped += 1;
            continue;
        }
        processed += 1;

        let response = capabilities.fetch.fetch(seed.url.as_str())?;
        let artifact = build_artifact(seed.url.clone(), response, capabilities.clock.now_epoch())?;
        let html = HtmlExtractor;
        let extractors: [&dyn Extractor; 1] = [&html];
        let result = run_extract(&artifact, &extractors, ExtractBudget::default(), &ExtractContext::default());

        let store_key = format!("pandark-wasm/page/{}", seed.url);
        match (&result.document, result.status) {
            (Some(document), status) if status != ExtractStatus::Failed => {
                let payload = serde_json::to_vec(document).map_err(serialization_error)?;
                capabilities.store.put(&store_key, &payload)?;
                report.extracted += 1;
                report.committed += 1;
                pages.push(PageOutcome {
                    url: seed.url.to_string(),
                    final_url: artifact.final_url.to_string(),
                    status,
                    store_key: Some(store_key),
                    stored: Some(StoredPageKind::Document),
                });
            }
            _ => {
                report.failed += 1;
                pages.push(PageOutcome {
                    url: seed.url.to_string(),
                    final_url: artifact.final_url.to_string(),
                    status: result.status,
                    store_key: None,
                    stored: None,
                });
            }
        }
    }

    let report_json = serde_json::to_string(&report).map_err(serialization_error)?;
    capabilities.store.put(&format!("pandark-wasm/report/{}", crawl_request.seeds[0].url), report_json.as_bytes())?;
    Ok(RunOutcome { report, report_json, pages })
}

fn build_artifact(
    request_url: Url,
    response: crate::capability::FetchResponse,
    retrieved_at_epoch: u64,
) -> Result<FetchArtifact, CapabilityError> {
    let final_url = Url::parse(&response.final_url)
        .map_err(|error| CapabilityError::new("fetch", format!("invalid final URL `{}`: {error}", response.final_url)))?;
    Ok(FetchArtifact {
        request_url: request_url.clone(),
        final_url,
        status: response.status,
        response_headers: response.headers,
        content_type: response.content_type,
        body: BodyStorage::inline(response.body),
        redirect_chain: Vec::<RedirectHop>::new(),
        retrieved_at_epoch,
        transport_provenance: response.provenance,
        cache_key: format!("wasm:{request_url}"),
    })
}

fn crawl_error(error: CrawlError) -> CapabilityError {
    CapabilityError::new("request", error.to_string())
}

fn serialization_error(error: serde_json::Error) -> CapabilityError {
    CapabilityError::new("store", error.to_string())
}
