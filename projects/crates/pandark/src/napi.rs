//! Node-API export surface for Pandark.

use std::path::PathBuf;

use napi::bindgen_prelude::*;
use napi_derive::napi;
use crate::{
    ChallengeBlocked, Extractor, HtmlExtractor, LoadedInput, initial_report, inspect_artifact,
    inspect_snapshot, load_input, parse_input_format, plan_crawl as build_plan, run_crawl,
    run_extract, run_extract_from_snapshot,
};
use crate::fetch::{
    FetchClient, FetchRequest, RobotsPolicy, Transport,
    transport_for_url,
};
use pandark_types::PolitenessProfile;
use pandark_types::{
    ChallengePolicy, CrawlRequest, ExtractBudget, ExtractContext, ExtractStatus,
    InspectStage,
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

/// N-API inspect response.
#[napi(object)]
pub struct InspectResponse {
    pub exit_code: u32,
    pub report_json: String,
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

/// Extract semantic IR from a local HTML file or browser snapshot.
#[napi]
pub fn extract_input(
    input_path: String,
    from: Option<String>,
    _source_url: Option<String>,
    challenge_policy: Option<String>,
) -> Result<ExtractResponse> {
    let format = parse_input_format(from.as_deref()).map_err(map_crawl_error)?;
    let loaded = load_input(&input_path, format).map_err(map_crawl_error)?;
    let policy = parse_challenge_policy(challenge_policy.as_deref())?;
    let html = HtmlExtractor;
    let extractors: [&dyn Extractor; 1] = [&html];
    let budget = ExtractBudget::default();
    let ctx = ExtractContext::default();

    match loaded {
        LoadedInput::Artifact(artifact) => {
            let result = run_extract(&artifact, &extractors, budget, &ctx);
            build_extract_response(result)
        }
        LoadedInput::Snapshot(snapshot) => match run_extract_from_snapshot(
            &snapshot,
            policy,
            &extractors,
            budget,
            &ctx,
        ) {
            Ok(result) => build_extract_response(result),
            Err(blocked) => build_challenge_extract_response(&snapshot, blocked),
        },
    }
}

/// Extract semantic IR from a local HTML file.
#[napi]
pub fn extract_file(input_path: String, source_url: Option<String>) -> Result<ExtractResponse> {
    extract_input(input_path, Some("html".into()), source_url, None)
}

/// Inspect local HTML or a browser snapshot without writing IR.
#[napi]
pub fn inspect_input(
    input_path: String,
    stage: Option<String>,
    from: Option<String>,
    challenge_policy: Option<String>,
) -> Result<InspectResponse> {
    let format = parse_input_format(from.as_deref()).map_err(map_crawl_error)?;
    let loaded = load_input(&input_path, format).map_err(map_crawl_error)?;
    let inspect_stage = parse_inspect_stage(stage.as_deref())?;
    let policy = parse_challenge_policy(challenge_policy.as_deref())?;
    let html = HtmlExtractor;
    let extractors: [&dyn Extractor; 1] = [&html];
    let budget = ExtractBudget::default();
    let report = match loaded {
        LoadedInput::Artifact(artifact) => inspect_artifact(&artifact, &extractors, inspect_stage, budget),
        LoadedInput::Snapshot(snapshot) => {
            inspect_snapshot(&snapshot, policy, &extractors, inspect_stage, budget)
        }
    };
    Ok(InspectResponse {
        exit_code: 0,
        report_json: serde_json::to_string(&report).map_err(map_serde_error)?,
    })
}

/// Inspect a local HTML file without writing IR.
#[napi]
pub fn inspect_file(input_path: String, stage: Option<String>) -> Result<InspectResponse> {
    inspect_input(input_path, stage, Some("html".into()), None)
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

fn parse_challenge_policy(policy: Option<&str>) -> Result<ChallengePolicy> {
    match policy.unwrap_or("stop") {
        "stop" => Ok(ChallengePolicy::Stop),
        "pause_for_operator" | "pause-for-operator" => Ok(ChallengePolicy::PauseForOperator),
        "use_existing_session" | "use-existing-session" => Ok(ChallengePolicy::UseExistingSession),
        "fallback_http" | "fallback-http" => Ok(ChallengePolicy::FallbackHttp),
        "skip_page" | "skip-page" => Ok(ChallengePolicy::SkipPage),
        other => Err(Error::from_reason(format!("unsupported challenge policy `{other}`"))),
    }
}

fn build_challenge_extract_response(
    snapshot: &pandark_types::BrowserSnapshot,
    blocked: ChallengeBlocked,
) -> Result<ExtractResponse> {
    let report_json = serde_json::json!({
        "schema_version": "pandark.report/v1",
        "operation": "extract",
        "status": "challenge_blocked",
        "source_url": snapshot.final_url.to_string(),
        "challenge_state": snapshot.challenge_state,
        "challenge_outcome": blocked.outcome,
    })
    .to_string();
    Ok(ExtractResponse {
        exit_code: 2,
        status: "challenge_blocked".into(),
        document_json: None,
        report_json,
    })
}

fn parse_inspect_stage(stage: Option<&str>) -> Result<InspectStage> {
    match stage.unwrap_or("probe") {
        "probe" => Ok(InspectStage::Probe),
        "links" => Ok(InspectStage::Links),
        "extract-plan" | "extract_plan" => Ok(InspectStage::ExtractPlan),
        other => Err(Error::from_reason(format!("unsupported inspect stage `{other}`"))),
    }
}

fn map_crawl_error(error: pandark_types::CrawlError) -> Error {
    Error::from_reason(error.to_string())
}

fn map_serde_error(error: serde_json::Error) -> Error {
    Error::from_reason(error.to_string())
}
