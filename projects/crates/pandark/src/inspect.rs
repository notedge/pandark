use pandark_types::{
    apply_challenge_policy, BrowserSnapshot, ChallengePolicy, ExtractBudget, ExtractContext,
    ExtractStatus, FetchArtifact, InspectReport, InspectStage, ProbeStatus,
};

use crate::extract::{Extractor, link_candidates_from_hrefs, run_extract, select_extractor};
use crate::extract::extract_hrefs;

/// Inspect a fetch artifact without committing IR.
pub fn inspect_artifact(
    artifact: &FetchArtifact,
    extractors: &[&dyn Extractor],
    stage: InspectStage,
    budget: ExtractBudget,
) -> InspectReport {
    let mut report = InspectReport::empty(artifact.final_url.clone(), stage);
    report.content_type = artifact.content_type.clone();
    report.body_bytes = artifact.body_bytes().map(|body| body.len() as u64).unwrap_or(0);

    if matches!(stage, InspectStage::Probe | InspectStage::ExtractPlan) {
        if let Some((_, probe)) = select_extractor(extractors, artifact, budget) {
            report.probe = probe;
        }
    }

    if matches!(stage, InspectStage::Links | InspectStage::ExtractPlan) {
        if let Some(bytes) = artifact.body_bytes() {
            let html = std::str::from_utf8(bytes).unwrap_or("");
            let hrefs = extract_hrefs(html);
            report.discovered_links =
                link_candidates_from_hrefs(&artifact.final_url, &hrefs);
        }
    }

    if stage == InspectStage::ExtractPlan {
        let result = run_extract(artifact, extractors, budget, &ExtractContext::default());
        if result.status == ExtractStatus::Complete || result.status == ExtractStatus::Partial {
            report.probe.status = ProbeStatus::Supported;
            report.probe.extractor = Some(result.extractor);
            if report.discovered_links.is_empty() {
                report.discovered_links = result.discovered_links;
            }
        }
    }

    report
}

/// Inspect a browser snapshot without committing IR.
pub fn inspect_snapshot(
    snapshot: &BrowserSnapshot,
    policy: ChallengePolicy,
    extractors: &[&dyn Extractor],
    stage: InspectStage,
    budget: ExtractBudget,
) -> InspectReport {
    let mut report = inspect_artifact(
        &crate::browser::snapshot_to_fetch_artifact(snapshot),
        extractors,
        stage,
        budget,
    );
    report.challenge_state = Some(snapshot.challenge_state);
    report.challenge_outcome = Some(apply_challenge_policy(snapshot.challenge_state, policy));
    report
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use pandark_types::{BodyStorage, TransportProvenance};
    use url::Url;

    use super::*;
    use crate::HtmlExtractor;

    fn html_artifact(html: &str) -> FetchArtifact {
        FetchArtifact {
            request_url: Url::parse("https://example.com/").expect("url"),
            final_url: Url::parse("https://example.com/").expect("url"),
            status: 200,
            response_headers: BTreeMap::from([("content-type".into(), "text/html".into())]),
            content_type: Some("text/html".into()),
            body: BodyStorage::inline(html.as_bytes().to_vec()),
            redirect_chain: Vec::new(),
            retrieved_at_epoch: 1,
            transport_provenance: TransportProvenance::Test,
            cache_key: "https://example.com/".into(),
        }
    }

    #[test]
    fn inspect_links_stage_finds_hrefs() {
        let artifact = html_artifact(
            r#"<html><body><a href="/next">next</a></body></html>"#,
        );
        let html = HtmlExtractor;
        let extractors: [&dyn Extractor; 1] = [&html];
        let report = inspect_artifact(
            &artifact,
            &extractors,
            InspectStage::Links,
            ExtractBudget::default(),
        );
        assert_eq!(report.discovered_links.len(), 1);
        assert!(report.discovered_links[0].target.contains("/next"));
    }
}
