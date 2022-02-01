//! WASM adapter for extract, inspect, and plan helpers.

#![deny(missing_docs)]

use pandark_extract::{Extractor, HtmlExtractor, inspect_artifact, run_extract};
use pandark_types::{
    CrawlReport, CrawlRequest, ExtractBudget, ExtractContext, ExtractStatus, FetchArtifact,
    InspectStage,
};
use wasm_bindgen::prelude::*;

/// Returns the Pandark WASM binding version.
#[wasm_bindgen]
pub fn pandark_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Extract semantic IR from a host-provided `FetchArtifact` JSON value.
#[wasm_bindgen]
pub fn extract_artifact_json(artifact_json: &str) -> Result<String, JsValue> {
    extract_artifact_json_str(artifact_json).map_err(|error| JsValue::from_str(&error))
}

/// Inspect a host-provided `FetchArtifact` JSON value without committing IR.
#[wasm_bindgen]
pub fn inspect_artifact_json(artifact_json: &str, stage: &str) -> Result<String, JsValue> {
    inspect_artifact_json_str(artifact_json, stage)
        .map_err(|error| JsValue::from_str(&error))
}

/// Build a crawl plan JSON envelope without performing network I/O.
#[wasm_bindgen]
pub fn plan_crawl_json(
    seed: &str,
    max_depth: Option<u32>,
    max_requests: Option<u32>,
) -> Result<String, JsValue> {
    plan_crawl_json_str(seed, max_depth, max_requests)
        .map_err(|error| JsValue::from_str(&error))
}

/// Extract semantic IR from a host-provided `FetchArtifact` JSON value.
pub fn extract_artifact_json_str(artifact_json: &str) -> Result<String, String> {
    let artifact: FetchArtifact = serde_json::from_str(artifact_json)
        .map_err(|error| format!("invalid fetch artifact json: {error}"))?;
    let html = HtmlExtractor;
    let extractors: [&dyn Extractor; 1] = [&html];
    let result = run_extract(
        &artifact,
        &extractors,
        ExtractBudget::default(),
        &ExtractContext::default(),
    );
    build_extract_response_json(&result)
}

/// Inspect a host-provided `FetchArtifact` JSON value without committing IR.
pub fn inspect_artifact_json_str(artifact_json: &str, stage: &str) -> Result<String, String> {
    let artifact: FetchArtifact = serde_json::from_str(artifact_json)
        .map_err(|error| format!("invalid fetch artifact json: {error}"))?;
    let inspect_stage = parse_inspect_stage(stage)?;
    let html = HtmlExtractor;
    let extractors: [&dyn Extractor; 1] = [&html];
    let report = inspect_artifact(
        &artifact,
        &extractors,
        inspect_stage,
        ExtractBudget::default(),
    );
    serde_json::to_string(&report).map_err(|error| error.to_string())
}

/// Build a crawl plan JSON envelope without performing network I/O.
pub fn plan_crawl_json_str(
    seed: &str,
    max_depth: Option<u32>,
    max_requests: Option<u32>,
) -> Result<String, String> {
    let mut request = CrawlRequest::from_seeds([seed])
        .map_err(|error| error.to_string())?;
    if let Some(depth) = max_depth {
        request.budget.max_depth = depth;
    }
    if let Some(requests) = max_requests {
        request.budget.max_requests = requests;
    }
    let seed_url = request
        .seeds
        .first()
        .ok_or_else(|| "at least one seed is required".to_string())?
        .url
        .clone();
    let report = CrawlReport::empty(seed_url);
    let frontier = request
        .seeds
        .iter()
        .map(|item| item.url.to_string())
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "schema_version": "pandark.plan/v1",
        "frontier": frontier,
        "report": report,
    })
    .to_string())
}

fn build_extract_response_json(result: &pandark_types::ExtractResult) -> Result<String, String> {
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
        .map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "schema_version": "pandark.report/v1",
        "operation": "extract",
        "status": status,
        "source_url": result.source_url.to_string(),
        "extractor": result.extractor,
        "loss_count": result.losses.len(),
        "discovered_links": result.discovered_links.len(),
        "document_json": document_json,
    })
    .to_string())
}

fn parse_inspect_stage(stage: &str) -> Result<InspectStage, String> {
    match stage {
        "probe" => Ok(InspectStage::Probe),
        "links" => Ok(InspectStage::Links),
        "extract-plan" | "extract_plan" => Ok(InspectStage::ExtractPlan),
        other => Err(format!("unsupported inspect stage `{other}`")),
    }
}

