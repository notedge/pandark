//! Semantic extraction over fetch artifacts.

#![deny(missing_docs)]

use pandark_types::{
    ExtractBudget, ExtractContext, ExtractInput, ExtractResult, ExtractStatus, FetchArtifact,
    LinkCandidate, LinkRelation, ProbeResult, ProbeStatus,
};

mod html;
mod inspect;

pub use html::HtmlExtractor;
pub use html::extract_hrefs;
pub use inspect::inspect_artifact;

/// Semantic extractor over fetch artifacts.
pub trait Extractor {
    /// Budgeted media-type probe.
    fn probe(&self, artifact: &FetchArtifact, budget: ExtractBudget) -> ProbeResult;
    /// Construct a document graph and discovered links.
    fn extract(&self, input: ExtractInput<'_>, ctx: &ExtractContext) -> ExtractResult;
}

/// Pick the first extractor that supports the artifact.
pub fn select_extractor<'a>(
    extractors: &'a [&'a dyn Extractor],
    artifact: &FetchArtifact,
    budget: ExtractBudget,
) -> Option<(&'a dyn Extractor, ProbeResult)> {
    for extractor in extractors {
        let probe = (*extractor).probe(artifact, budget);
        if probe.status == ProbeStatus::Supported {
            return Some((*extractor, probe));
        }
    }
    None
}

/// Run offline extraction against a fetch artifact.
pub fn run_extract(
    artifact: &FetchArtifact,
    extractors: &[&dyn Extractor],
    budget: ExtractBudget,
    ctx: &ExtractContext,
) -> ExtractResult {
    let input = ExtractInput { artifact, budget };
    if let Some((extractor, _)) = select_extractor(extractors, artifact, budget) {
        return extractor.extract(input, ctx);
    }
    ExtractResult {
        status: ExtractStatus::Unsupported,
        source_url: artifact.final_url.clone(),
        document: None,
        discovered_links: Vec::new(),
        losses: Vec::new(),
        extractor: "none".into(),
    }
}

/// Classify a discovered href relative to a page URL.
pub fn classify_link(page_url: &url::Url, target: &str) -> (LinkRelation, String) {
    let resolved = page_url.join(target).map(|url| url.to_string());
    match resolved {
        Ok(absolute) => {
            let relation = match url::Url::parse(&absolute) {
                Ok(parsed) if page_url.host_str() == parsed.host_str() => LinkRelation::Navigation,
                _ => LinkRelation::External,
            };
            (relation, absolute)
        }
        Err(_) => (LinkRelation::External, target.to_string()),
    }
}

/// Build link candidates from raw href strings.
pub fn link_candidates_from_hrefs(page_url: &url::Url, hrefs: &[String]) -> Vec<LinkCandidate> {
    let mut links = Vec::new();
    for href in hrefs {
        let (relation, target) = classify_link(page_url, href);
        links.push(LinkCandidate {
            target,
            relation,
            source_hint: Some("href".into()),
        });
    }
    links
}
