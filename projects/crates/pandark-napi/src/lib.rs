//! Node-API export surface for Pandark.

use std::path::PathBuf;

use napi::bindgen_prelude::*;
use napi_derive::napi;
use notedown_formats::{export::markdown::export_markdown, import::html::import_html_bytes_with_links};
use pandark::{
    BrowserProvider, ChallengeBlocked, CrawlOptions, CrawlOutput, Extractor, HtmlExtractor, LoadedInput, build_browser_stack,
    initial_report, inspect_artifact, inspect_snapshot, load_input, parse_input_format, plan_crawl as build_plan,
    resolve_crawl_seed_specs, resume_crawl_from_checkpoint, run_crawl_with_options, run_extract, run_extract_from_snapshot,
};
use pandark_fetch::{FetchClient, FetchRequest, RobotsPolicy, transport_for_url};
use pandark_types::{
    BrowserFallbackPolicy, ChallengeOutcome, ChallengePolicy, CrawlCheckpoint, CrawlReport, CrawlRequest, CrawlScope,
    ExtractBudget, ExtractContext, ExtractStatus, InspectStage, PolitenessProfile,
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

/// One fetched page returned alongside a crawl report.
#[napi(object)]
pub struct CrawlPageArtifact {
    /// Request URL before redirects (also the crawl page identity).
    pub requested_url: String,
    /// Final URL after redirects.
    pub final_url: String,
    /// Final HTTP status.
    pub status: u32,
    /// Parsed media type when known.
    pub content_type: Option<String>,
    /// Response headers serialized as a JSON object.
    pub headers_json: String,
    /// Inline response body bytes.
    pub body: Buffer,
}

/// N-API local crawl response.
#[napi(object)]
pub struct CrawlResponse {
    pub exit_code: u32,
    pub report_json: String,
    pub committed_pages: Vec<String>,
    pub checkpoint_json: Option<String>,
    /// Every page fetched during the run, in crawl order.
    pub pages: Vec<CrawlPageArtifact>,
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

/// One-shot HTML → Markdown result from Notedown IR.
#[napi(object)]
pub struct MarkdownResult {
    pub markdown: String,
    pub diagnostic_namespace: String,
    pub diagnostic_codes: Vec<String>,
}

/// Returns the Pandark N-API binding version.
#[napi]
#[allow(missing_docs)]
pub fn pandark_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Build a crawl plan without network I/O.
#[napi]
pub fn plan_crawl(seed: String, max_depth: Option<u32>, max_requests: Option<u32>) -> Result<PlanResponse> {
    let request = parse_request(&seed, max_depth, max_requests)?;
    let plan = build_plan(request.clone()).map_err(map_crawl_error)?;
    let report = initial_report(&request).map_err(map_crawl_error)?;
    Ok(PlanResponse { frontier: plan.frontier, report_json: serde_json::to_string(&report).map_err(map_serde_error)? })
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
        LoadedInput::Snapshot(snapshot) => match run_extract_from_snapshot(&snapshot, policy, &extractors, budget, &ctx) {
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
        LoadedInput::Snapshot(snapshot) => inspect_snapshot(&snapshot, policy, &extractors, inspect_stage, budget),
    };
    let exit_code = match report.challenge_outcome {
        Some(ChallengeOutcome::Stop) => 2,
        Some(ChallengeOutcome::PauseForOperator) => 3,
        _ => 0,
    };
    Ok(InspectResponse { exit_code, report_json: serde_json::to_string(&report).map_err(map_serde_error)? })
}

/// Inspect a local HTML file without writing IR.
#[napi]
pub fn inspect_file(input_path: String, stage: Option<String>) -> Result<InspectResponse> {
    inspect_input(input_path, stage, Some("html".into()), None)
}

/// Fetch one URL through the default transport for its scheme.
///
/// Resolves a Promise: the fetch runs on the Node-API thread pool and never
/// blocks the JavaScript event loop.
#[napi]
pub fn fetch_seed(seed: String) -> AsyncTask<FetchSeedTask> {
    AsyncTask::new(FetchSeedTask { seed })
}

/// Thread-pool task backing [`fetch_seed`].
pub struct FetchSeedTask {
    seed: String,
}

impl Task for FetchSeedTask {
    type Output = FetchResponse;
    type JsValue = FetchResponse;

    fn compute(&mut self) -> Result<Self::Output> {
        fetch_seed_response(&self.seed)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

fn fetch_seed_response(seed: &str) -> Result<FetchResponse> {
    let seed_url = normalize_seed_url(seed)?;
    let url = Url::parse(&seed_url).map_err(|error| Error::from_reason(error.to_string()))?;
    let transport = transport_for_url(&url).map_err(map_crawl_error)?;
    let mut client = FetchClient::new(transport, RobotsPolicy::from_profile(PolitenessProfile::Conservative));
    let artifact = client.fetch(&FetchRequest { url, headers: Default::default() }).map_err(map_crawl_error)?;
    Ok(FetchResponse { exit_code: 0, artifact_json: serde_json::to_string(&artifact).map_err(map_serde_error)? })
}

/// Crawl linked HTML pages through the transport implied by the seed URL.
///
/// Resolves a Promise: the crawl runs on the Node-API thread pool and never
/// blocks the JavaScript event loop.
#[napi]
#[allow(clippy::too_many_arguments)]
pub fn crawl_file(
    seed: String,
    max_depth: Option<u32>,
    max_requests: Option<u32>,
    browser_fixtures_dir: Option<String>,
    browser_fallback: Option<String>,
    challenge_policy: Option<String>,
    browser_endpoint: Option<String>,
    cache_dir: Option<String>,
    scope_hosts: Option<Vec<String>>,
    scope_domains: Option<Vec<String>>,
    politeness: Option<String>,
) -> AsyncTask<CrawlFileTask> {
    AsyncTask::new(CrawlFileTask {
        seed,
        max_depth,
        max_requests,
        browser_fixtures_dir,
        browser_fallback,
        challenge_policy,
        browser_endpoint,
        cache_dir,
        scope_hosts,
        scope_domains,
        politeness,
    })
}

/// Thread-pool task backing [`crawl_file`].
pub struct CrawlFileTask {
    seed: String,
    max_depth: Option<u32>,
    max_requests: Option<u32>,
    browser_fixtures_dir: Option<String>,
    browser_fallback: Option<String>,
    challenge_policy: Option<String>,
    browser_endpoint: Option<String>,
    cache_dir: Option<String>,
    scope_hosts: Option<Vec<String>>,
    scope_domains: Option<Vec<String>>,
    politeness: Option<String>,
}

impl Task for CrawlFileTask {
    type Output = CrawlResponse;
    type JsValue = CrawlResponse;

    fn compute(&mut self) -> Result<Self::Output> {
        crawl_file_response(
            &self.seed,
            self.max_depth,
            self.max_requests,
            self.browser_fixtures_dir.as_deref(),
            self.browser_fallback.as_deref(),
            self.challenge_policy.as_deref(),
            self.browser_endpoint.as_deref(),
            self.cache_dir.as_deref(),
            self.scope_hosts.clone(),
            self.scope_domains.clone(),
            self.politeness.as_deref(),
        )
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

#[allow(clippy::too_many_arguments)]
fn crawl_file_response(
    seed: &str,
    max_depth: Option<u32>,
    max_requests: Option<u32>,
    browser_fixtures_dir: Option<&str>,
    browser_fallback: Option<&str>,
    challenge_policy: Option<&str>,
    browser_endpoint: Option<&str>,
    cache_dir: Option<&str>,
    scope_hosts: Option<Vec<String>>,
    scope_domains: Option<Vec<String>>,
    politeness: Option<&str>,
) -> Result<CrawlResponse> {
    let cache_path = cache_dir.map(PathBuf::from);
    let mut request = parse_request(seed, max_depth, max_requests)?;
    apply_request_options(&mut request, scope_hosts, scope_domains, politeness)?;
    let seed_url = request.seeds.first().ok_or_else(|| Error::from_reason("at least one seed is required"))?;
    let transport = transport_for_url(&seed_url.url).map_err(map_crawl_error)?;
    let html = HtmlExtractor;
    let extractors: [&dyn Extractor; 1] = [&html];

    let browser_stack = build_browser_stack(browser_fixtures_dir, browser_endpoint).map_err(map_crawl_error)?;
    let options = CrawlOptions {
        browser: if browser_stack.is_empty() { None } else { Some(&browser_stack as &dyn BrowserProvider) },
        browser_fallback: parse_browser_fallback(browser_fallback)?,
        challenge_policy: parse_challenge_policy(challenge_policy)?,
        cache_dir: cache_path.as_deref(),
        ..Default::default()
    };

    let output = run_crawl_with_options(request, transport, &extractors, options).map_err(map_crawl_error)?;
    build_crawl_response(output)
}

/// Resume a crawl from a serialized checkpoint file.
///
/// Resolves a Promise: the resume runs on the Node-API thread pool and never
/// blocks the JavaScript event loop.
#[napi]
pub fn resume_crawl_file(
    checkpoint_json: String,
    browser_fixtures_dir: Option<String>,
    browser_fallback: Option<String>,
    challenge_policy: Option<String>,
    browser_endpoint: Option<String>,
    cache_dir: Option<String>,
) -> AsyncTask<ResumeCrawlFileTask> {
    AsyncTask::new(ResumeCrawlFileTask {
        checkpoint_json,
        browser_fixtures_dir,
        browser_fallback,
        challenge_policy,
        browser_endpoint,
        cache_dir,
    })
}

/// Thread-pool task backing [`resume_crawl_file`].
pub struct ResumeCrawlFileTask {
    checkpoint_json: String,
    browser_fixtures_dir: Option<String>,
    browser_fallback: Option<String>,
    challenge_policy: Option<String>,
    browser_endpoint: Option<String>,
    cache_dir: Option<String>,
}

impl Task for ResumeCrawlFileTask {
    type Output = CrawlResponse;
    type JsValue = CrawlResponse;

    fn compute(&mut self) -> Result<Self::Output> {
        resume_crawl_file_response(
            &self.checkpoint_json,
            self.browser_fixtures_dir.as_deref(),
            self.browser_fallback.as_deref(),
            self.challenge_policy.as_deref(),
            self.browser_endpoint.as_deref(),
            self.cache_dir.as_deref(),
        )
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

fn resume_crawl_file_response(
    checkpoint_json: &str,
    browser_fixtures_dir: Option<&str>,
    browser_fallback: Option<&str>,
    challenge_policy: Option<&str>,
    browser_endpoint: Option<&str>,
    cache_dir: Option<&str>,
) -> Result<CrawlResponse> {
    let cache_path = cache_dir.map(PathBuf::from);
    let checkpoint: CrawlCheckpoint = serde_json::from_str(checkpoint_json).map_err(map_serde_error)?;
    let seed_url = checkpoint.request.seeds.first().ok_or_else(|| Error::from_reason("checkpoint request has no seeds"))?;
    let transport = transport_for_url(&seed_url.url).map_err(map_crawl_error)?;
    let html = HtmlExtractor;
    let extractors: [&dyn Extractor; 1] = [&html];

    let browser_stack = build_browser_stack(browser_fixtures_dir, browser_endpoint).map_err(map_crawl_error)?;
    let options = CrawlOptions {
        browser: if browser_stack.is_empty() { None } else { Some(&browser_stack as &dyn BrowserProvider) },
        browser_fallback: parse_browser_fallback(browser_fallback)?,
        challenge_policy: parse_challenge_policy(challenge_policy)?,
        cache_dir: cache_path.as_deref(),
        ..Default::default()
    };

    let output = resume_crawl_from_checkpoint(checkpoint, transport, &extractors, options).map_err(map_crawl_error)?;
    build_crawl_response(output)
}

fn build_crawl_response(output: CrawlOutput) -> Result<CrawlResponse> {
    let exit_code = crawl_exit_code(&output.report, output.checkpoint.is_some());
    let checkpoint_json =
        output.checkpoint.as_ref().map(|checkpoint| serde_json::to_string(checkpoint)).transpose().map_err(map_serde_error)?;
    let pages = output
        .artifacts
        .iter()
        .map(|artifact| {
            Ok(CrawlPageArtifact {
                requested_url: artifact.request_url.to_string(),
                final_url: artifact.final_url.to_string(),
                status: u32::from(artifact.status),
                content_type: artifact.content_type.clone(),
                headers_json: serde_json::to_string(&artifact.response_headers).map_err(map_serde_error)?,
                body: Buffer::from(artifact.body_bytes().unwrap_or_default().to_vec()),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(CrawlResponse {
        exit_code,
        report_json: serde_json::to_string(&output.report).map_err(map_serde_error)?,
        committed_pages: output.committed_pages,
        checkpoint_json,
        pages,
    })
}

/// Apply the TS authoring options that the crawl request carries.
fn apply_request_options(
    request: &mut CrawlRequest,
    scope_hosts: Option<Vec<String>>,
    scope_domains: Option<Vec<String>>,
    politeness: Option<&str>,
) -> Result<()> {
    if let Some(politeness) = politeness {
        request.politeness = parse_politeness(politeness)?;
    }
    let scope =
        CrawlScope { allowed_hosts: scope_hosts.unwrap_or_default(), allowed_domains: scope_domains.unwrap_or_default() };
    if !scope.is_empty() {
        request.scope = Some(scope);
    }
    Ok(())
}

/// One-shot HTML bytes → Markdown string via Notedown IR.
#[napi]
pub fn extract_to_markdown(html_bytes: Vec<u8>, source_url: Option<String>) -> Result<MarkdownResult> {
    let label = source_url.unwrap_or_else(|| "inline-html".to_string());
    let import = import_html_bytes_with_links(&label, &html_bytes)
        .map_err(|err| Error::from_reason(format!("notedown-formats HTML import failed: {err}")))?;
    let graph = import.document;
    let markdown = export_markdown(&graph)
        .map_err(|err| Error::from_reason(format!("notedown-formats markdown export failed: {err}")))?;
    let diagnostic_codes = graph.coverage.loss.iter().map(|loss| loss.code.clone()).collect();
    Ok(MarkdownResult { markdown, diagnostic_namespace: "notedown.formats.html".into(), diagnostic_codes })
}

fn crawl_exit_code(report: &CrawlReport, awaiting_resume: bool) -> u32 {
    if awaiting_resume {
        3
    }
    else if report.failed > 0 {
        2
    }
    else if report.skipped > 0 {
        3
    }
    else {
        0
    }
}

fn parse_request(seed: &str, max_depth: Option<u32>, max_requests: Option<u32>) -> Result<CrawlRequest> {
    let seed_urls = resolve_crawl_seed_specs(seed).map_err(map_crawl_error)?;
    let mut request = CrawlRequest::from_seeds(seed_urls).map_err(map_crawl_error)?;
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
    let url = Url::from_file_path(path).map_err(|_| Error::from_reason(format!("invalid crawl seed path or URL `{seed}`")))?;
    Ok(url.to_string())
}

fn build_extract_response(result: pandark_types::ExtractResult) -> Result<ExtractResponse> {
    let status = match result.status {
        ExtractStatus::Complete => "complete",
        ExtractStatus::Partial => "partial",
        ExtractStatus::Unsupported => "unsupported",
        ExtractStatus::Failed => "failed",
    };
    let document_json =
        result.document.as_ref().map(|graph| serde_json::to_string(graph)).transpose().map_err(map_serde_error)?;
    let exit_code = match result.status {
        ExtractStatus::Complete => 0,
        ExtractStatus::Partial => 3,
        ExtractStatus::Unsupported | ExtractStatus::Failed => 2,
    };
    let report_json = serde_json::json!({
        "schema_version": pandark_types::diagnostics::SCHEMA_REPORT,
        "operation": "extract",
        "status": status,
        "source_url": result.source_url.to_string(),
        "extractor": result.extractor,
        "loss_count": result.losses.len(),
        "discovered_links": result.discovered_links.len(),
    })
    .to_string();
    Ok(ExtractResponse { exit_code, status: status.to_string(), document_json, report_json })
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

fn parse_browser_fallback(policy: Option<&str>) -> Result<BrowserFallbackPolicy> {
    match policy.unwrap_or("never") {
        "never" => Ok(BrowserFallbackPolicy::Never),
        "on_fetch_failure" | "on-fetch-failure" => Ok(BrowserFallbackPolicy::OnFetchFailure),
        other => Err(Error::from_reason(format!("unsupported browser fallback `{other}`"))),
    }
}

fn parse_politeness(profile: &str) -> Result<PolitenessProfile> {
    match profile {
        "conservative" => Ok(PolitenessProfile::Conservative),
        "balanced" => Ok(PolitenessProfile::Balanced),
        "aggressive" | "developer" => Ok(PolitenessProfile::Developer),
        other => Err(Error::from_reason(format!("unsupported politeness profile `{other}`"))),
    }
}

fn build_challenge_extract_response(
    snapshot: &pandark_types::BrowserSnapshot,
    blocked: ChallengeBlocked,
) -> Result<ExtractResponse> {
    let report_json = serde_json::json!({
        "schema_version": pandark_types::diagnostics::SCHEMA_REPORT,
        "operation": "extract",
        "status": "challenge_blocked",
        "source_url": snapshot.final_url.to_string(),
        "challenge_state": snapshot.challenge_state,
        "challenge_outcome": blocked.outcome,
    })
    .to_string();
    Ok(ExtractResponse { exit_code: 2, status: "challenge_blocked".into(), document_json: None, report_json })
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
