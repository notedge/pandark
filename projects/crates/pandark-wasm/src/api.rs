use pandark_types::{
    CrawlError, CrawlReport, CrawlRequest, ExtractBudget, FetchArtifact, InspectReport,
    InspectStage, LinkCandidate, LinkRelation, ProbeStatus,
};
use url::Url;

/// Build a crawl plan JSON envelope without filesystem or network access.
pub fn plan_crawl_json(
    seed: &str,
    max_depth: Option<u32>,
    max_requests: Option<u32>,
) -> Result<String, CrawlError> {
    let mut request = parse_network_request(seed)?;
    if let Some(depth) = max_depth {
        request.budget.max_depth = depth;
    }
    if let Some(requests) = max_requests {
        request.budget.max_requests = requests;
    }
    let frontier = request
        .seeds
        .iter()
        .map(|seed| seed.url.to_string())
        .collect::<Vec<_>>();
    let report = initial_report(&request)?;
    let payload = serde_json::json!({
        "frontier": frontier,
        "report": report,
    });
    serde_json::to_string(&payload).map_err(|error| CrawlError::InvalidInput(error.to_string()))
}

/// Probe and link-discover an artifact provided by the host fetch capability.
pub fn inspect_artifact_json(
    artifact_json: &str,
    stage: &str,
) -> Result<String, CrawlError> {
    let artifact: FetchArtifact = serde_json::from_str(artifact_json)
        .map_err(|error| CrawlError::InvalidInput(error.to_string()))?;
    let inspect_stage = parse_inspect_stage(stage)?;
    let report = inspect_artifact(&artifact, inspect_stage);
    serde_json::to_string(&report).map_err(|error| CrawlError::InvalidInput(error.to_string()))
}

/// Return a minimal extract report for a host-provided artifact.
pub fn extract_artifact_json(artifact_json: &str) -> Result<String, CrawlError> {
    let artifact: FetchArtifact = serde_json::from_str(artifact_json)
        .map_err(|error| CrawlError::InvalidInput(error.to_string()))?;
    let report = inspect_artifact(&artifact, InspectStage::ExtractPlan);
    let status = if report.probe.status == ProbeStatus::Supported {
        "partial"
    } else {
        "unsupported"
    };
    let payload = serde_json::json!({
        "schema_version": "pandark.report/v1",
        "operation": "extract",
        "status": status,
        "source_url": artifact.final_url.to_string(),
        "extractor": report.probe.extractor,
        "discovered_links": report.discovered_links.len(),
        "loss_count": 0,
    });
    serde_json::to_string(&payload).map_err(|error| CrawlError::InvalidInput(error.to_string()))
}

fn parse_network_request(seed: &str) -> Result<CrawlRequest, CrawlError> {
    let trimmed = seed.trim();
    if trimmed.is_empty() {
        return Err(CrawlError::InvalidInput("crawl seed input is empty".into()));
    }
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err(CrawlError::InvalidInput(
            "wasm plan_crawl accepts http or https seeds only".into(),
        ));
    }
    CrawlRequest::from_seeds([trimmed])
}

fn initial_report(request: &CrawlRequest) -> Result<CrawlReport, CrawlError> {
    let seed = request
        .seeds
        .first()
        .ok_or_else(|| CrawlError::InvalidInput("at least one seed is required".into()))?
        .url
        .clone();
    Ok(CrawlReport::empty(seed))
}

fn parse_inspect_stage(stage: &str) -> Result<InspectStage, CrawlError> {
    match stage {
        "probe" => Ok(InspectStage::Probe),
        "links" => Ok(InspectStage::Links),
        "extract-plan" | "extract_plan" => Ok(InspectStage::ExtractPlan),
        other => Err(CrawlError::InvalidInput(format!(
            "unsupported inspect stage `{other}`"
        ))),
    }
}

fn inspect_artifact(artifact: &FetchArtifact, stage: InspectStage) -> InspectReport {
    let mut report = InspectReport::empty(artifact.final_url.clone(), stage);
    report.content_type = artifact.content_type.clone();
    report.body_bytes = artifact.body_bytes().map(|body| body.len() as u64).unwrap_or(0);
    report.probe = probe_artifact(artifact);

    if matches!(stage, InspectStage::Links | InspectStage::ExtractPlan) {
        if let Some(bytes) = artifact.body_bytes() {
            let html = String::from_utf8_lossy(bytes);
            report.discovered_links =
                link_candidates_from_hrefs(&artifact.final_url, discover_hrefs(&html));
        }
    }

    report
}

fn probe_artifact(artifact: &FetchArtifact) -> pandark_types::ProbeResult {
    let supported = artifact
        .content_type
        .as_deref()
        .map(|value| value.contains("html"))
        .unwrap_or(false);
    if !supported {
        return pandark_types::ProbeResult {
            status: ProbeStatus::Unsupported,
            extractor: None,
        };
    }
    let bytes = artifact.body_bytes().unwrap_or(&[]);
    if bytes.len() as u64 > ExtractBudget::default().max_probe_bytes {
        return pandark_types::ProbeResult {
            status: ProbeStatus::Failed,
            extractor: Some("html".into()),
        };
    }
    pandark_types::ProbeResult {
        status: ProbeStatus::Supported,
        extractor: Some("html".into()),
    }
}

fn discover_hrefs(html: &str) -> Vec<String> {
    let mut hrefs = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find("href=\"") {
        rest = &rest[start + 6..];
        if let Some(end) = rest.find('"') {
            let href = rest[..end].trim();
            if !href.is_empty() {
                hrefs.push(href.to_string());
            }
            rest = &rest[end + 1..];
        } else {
            break;
        }
    }
    hrefs
}

fn link_candidates_from_hrefs(base: &Url, hrefs: Vec<String>) -> Vec<LinkCandidate> {
    hrefs
        .into_iter()
        .filter_map(|href| base.join(&href).ok())
        .map(|url| LinkCandidate {
            target: url.to_string(),
            relation: LinkRelation::Navigation,
            source_hint: None,
        })
        .collect()
}
