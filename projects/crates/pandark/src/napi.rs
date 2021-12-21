//! Node-API export surface for Pandark.

use std::path::PathBuf;

use napi::bindgen_prelude::*;
use napi_derive::napi;
use crate::{
    Extractor, HtmlExtractor, initial_report, plan_crawl as build_plan, run_crawl, run_extract,
};
use crate::fetch::{
    FetchClient, FetchRequest, FileTransport, RobotsPolicy, Transport,
    transport_for_url,
};
use pandark_types::PolitenessProfile;
use pandark_types::{
    CrawlRequest, ExtractBudget, ExtractContext, ExtractStatus, FetchArtifact,
};
use url::Url;

/// N-API crawl plan response.
#[napi(object)]
pub struct PlanResponse {
    pub frontier: Vec<String>,
    pub report_json: String,
}

/// N-API offline extract response.
#[napi(object)]
pub struct ExtractResponse {
    pub exit_code: u32,
    pub status: String,
    pub document_json: Option<String>,
    pub report_json: String,
}

/// N-API local crawl response.
#[napi(object)]
pub struct CrawlResponse {
    pub exit_code: u32,
    pub report_json: String,
    pub committed_pages: Vec<String>,
}

/// N-API fetch response.
#[napi(object)]
pub struct FetchResponse {
    pub exit_code: u32,
    pub artifact_json: String,
}

/// Returns the Pandark N-API binding version.
#[napi]
pub fn pandark_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Build a crawl plan without network I/O.
#[napi]
pub fn plan_crawl(
    seed: String,
    max_depth: Option<u32>,
    max_requests: Option<u32>,
) -> Result<PlanResponse> {
    let request = parse_request(&seed, max_depth, max_requests)?;
    let plan = build_plan(request.clone()).map_err(map_crawl_error)?;
    let report = initial_report(&request).map_err(map_crawl_error)?;
    Ok(PlanResponse {
        frontier: plan.frontier,
        report_json: serde_json::to_string(&report).map_err(map_serde_error)?,
    })
}

/// Extract semantic IR from a local HTML file.
#[napi]
pub fn extract_file(input_path: String, source_url: Option<String>) -> Result<ExtractResponse> {
    let artifact = artifact_from_path(&input_path, source_url.as_deref())?;
    let html = HtmlExtractor;
    let extractors: [&dyn Extractor; 1] = [&html];
    let result = run_extract(
        &artifact,
        &extractors,
        ExtractBudget::default(),
        &ExtractContext::default(),
    );
    build_extract_response(result)
}

/// Fetch one URL through the default transport for its scheme.
#[napi]
pub fn fetch_seed(seed: String) -> Result<FetchResponse> {
    let seed_url = normalize_seed_url(&seed)?;
    let url = Url::parse(&seed_url).map_err(|error| Error::from_reason(error.to_string()))?;
    let transport = transport_for_url(&url).map_err(map_crawl_error)?;
    let mut client = FetchClient::new(transport, RobotsPolicy::from_profile(PolitenessProfile::Conservative));
    let artifact = client
        .fetch(&FetchRequest {
            url,
            headers: Default::default(),
        })
        .map_err(map_crawl_error)?;
    Ok(FetchResponse {
        exit_code: 0,
        artifact_json: serde_json::to_string(&artifact).map_err(map_serde_error)?,
    })
}

/// Crawl linked HTML pages through the transport implied by the seed URL.
#[napi]
pub fn crawl_file(
    seed: String,
    max_depth: Option<u32>,
    max_requests: Option<u32>,
) -> Result<CrawlResponse> {
    let request = parse_request(&seed, max_depth, max_requests)?;
    let seed_url = request
        .seeds
        .first()
        .ok_or_else(|| Error::from_reason("at least one seed is required"))?;
    let transport = transport_for_url(&seed_url.url).map_err(map_crawl_error)?;
    let html = HtmlExtractor;
    let extractors: [&dyn Extractor; 1] = [&html];
    let output = run_crawl(request, transport, &extractors).map_err(map_crawl_error)?;
    let exit_code = if output.report.failed > 0 {
        2
    } else if output.report.skipped > 0 {
        3
    } else {
        0
    };
    Ok(CrawlResponse {
        exit_code,
        report_json: serde_json::to_string(&output.report).map_err(map_serde_error)?,
        committed_pages: output.committed_pages,
    })
}

fn parse_request(
    seed: &str,
    max_depth: Option<u32>,
    max_requests: Option<u32>,
) -> Result<CrawlRequest> {
    let seed_url = normalize_seed_url(seed)?;
    let mut request = CrawlRequest::from_seed(seed_url.as_str()).map_err(map_crawl_error)?;
    if let Some(depth) = max_depth {
        request.budget.max_depth = depth;
    }
    if let Some(requests) = max_requests {
        request.budget.max_requests = requests;
    }
    Ok(request)
}

fn normalize_seed_url(seed: &str) -> Result<String> {
    if seed.starts_with("file://") || seed.starts_with("http://") || seed.starts_with("https://") {
        return Ok(seed.to_string());
    }
    let path = PathBuf::from(seed);
    let url = Url::from_file_path(path).map_err(|_| {
        Error::from_reason(format!("invalid crawl seed path or URL `{seed}`"))
    })?;
    Ok(url.to_string())
}

fn artifact_from_path(input_path: &str, _source_url: Option<&str>) -> Result<FetchArtifact> {
    let path = PathBuf::from(input_path);
    let file_url = Url::from_file_path(path).map_err(|_| {
        Error::from_reason(format!("invalid extract input path `{input_path}`"))
    })?;
    let transport = FileTransport::new();
    let request = FetchRequest {
        url: file_url,
        headers: Default::default(),
    };
    transport.fetch(&request).map_err(map_crawl_error)
}

fn build_extract_response(result: pandark_types::ExtractResult) -> Result<ExtractResponse> {
    let status = match result.status {
        ExtractStatus::Complete => "complete",
        ExtractStatus::Partial => "partial",
        ExtractStatus::Unsupported => "unsupported",
        ExtractStatus::Failed => "failed",
    };
    let document_json = result
        .document
        .as_ref()
        .map(|graph| serde_json::to_string(graph))
        .transpose()
        .map_err(map_serde_error)?;
    let exit_code = match result.status {
        ExtractStatus::Complete => 0,
        ExtractStatus::Partial => 3,
        ExtractStatus::Unsupported | ExtractStatus::Failed => 2,
    };
    let report_json = serde_json::json!({
        "schema_version": "pandark.report/v1",
        "operation": "extract",
        "status": status,
        "source_url": result.source_url.to_string(),
        "extractor": result.extractor,
        "loss_count": result.losses.len(),
        "discovered_links": result.discovered_links.len(),
    })
    .to_string();
    Ok(ExtractResponse {
        exit_code,
        status: status.to_string(),
        document_json,
        report_json,
    })
}

fn map_crawl_error(error: pandark_types::CrawlError) -> Error {
    Error::from_reason(error.to_string())
}

fn map_serde_error(error: serde_json::Error) -> Error {
    Error::from_reason(error.to_string())
}
