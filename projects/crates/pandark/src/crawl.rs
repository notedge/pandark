use std::collections::BTreeMap;

use crate::fetch::{FetchClient, FetchRequest, RobotsPolicy, Transport};
use pandark_types::{
    AdmissionDecision, CrawlEvent, CrawlReport, CrawlRequest, ExtractBudget, ExtractContext,
    ExtractStatus, FetchArtifact, FrontierItem, Result,
};

use crate::admission::{admit_frontier_item, seed_frontier};
use crate::extract::{Extractor, run_extract};
use crate::frontier::{FrontierQueue, frontier_from_links};
use crate::initial_report;
use crate::transaction::commit_page;
use pandark_types::PageTransaction;

/// Mutable crawl session state for checkpointing.
#[derive(Debug, Clone)]
pub struct CrawlState {
    /// User request.
    pub request: CrawlRequest,
    /// Frontier queue.
    pub queue: FrontierQueue,
    /// Requests consumed so far.
    pub requests_used: u32,
    /// Accumulated report.
    pub report: CrawlReport,
}

/// Output of a crawl run.
#[derive(Debug, Clone)]
pub struct CrawlOutput {
    /// Final report.
    pub report: CrawlReport,
    /// Committed page identities.
    pub committed_pages: Vec<String>,
    /// Artifacts available for offline re-extract.
    pub artifacts: Vec<FetchArtifact>,
}

/// Run a bounded crawl using the provided transport.
pub fn run_crawl<T: Transport>(
    request: CrawlRequest,
    transport: T,
    extractors: &[&dyn Extractor],
) -> Result<CrawlOutput> {
    let mut state = CrawlState {
        request: request.clone(),
        queue: FrontierQueue::new(),
        requests_used: 0,
        report: initial_report(&request)?,
    };

    for item in seed_frontier(&request)? {
        state.report.events.push(CrawlEvent::Discovered {
            url: item.request_url.clone(),
            depth: item.depth,
        });
        state.queue.push(item);
    }

    let mut client = FetchClient::new(transport, RobotsPolicy::from_profile(request.politeness));
    let mut committed_pages = Vec::new();
    let mut artifacts = Vec::new();
    let extract_budget = ExtractBudget::default();

    while let Some(item) = state.queue.pop() {
        if state.requests_used >= request.budget.max_requests {
            record_skipped(&mut state, &item, "request-budget-exhausted");
            continue;
        }

        let decision = admit_frontier_item(&item, &request);
        state.report.events.push(CrawlEvent::Admitted {
            url: item.request_url.clone(),
            decision: decision.clone(),
        });
        if !matches!(decision, AdmissionDecision::Admitted) {
            state.report.skipped += 1;
            continue;
        }

        state.requests_used += 1;
        let fetch_request = FetchRequest {
            url: item.request_url.clone(),
            headers: BTreeMap::new(),
        };
        let artifact = match client.fetch(&fetch_request) {
            Ok(artifact) => artifact,
            Err(error) => {
                state.report.failed += 1;
                state.report.events.push(CrawlEvent::Failed {
                    url: item.request_url.clone(),
                    reason: error.to_string(),
                });
                continue;
            }
        };

        let cache_hit = artifact.transport_provenance
            == pandark_types::TransportProvenance::CacheHit;
        state.report.events.push(CrawlEvent::Fetched {
            url: artifact.final_url.clone(),
            status: artifact.status,
            cache_hit,
        });
        artifacts.push(artifact.clone());

        let extract = run_extract(
            &artifact,
            extractors,
            extract_budget,
            &ExtractContext {
                page_identity: Some(item.request_url.to_string()),
            },
        );
        state.report.events.push(CrawlEvent::Extracted {
            url: extract.source_url.clone(),
            status: extract.status,
            extractor: extract.extractor.clone(),
        });

        if matches!(extract.status, ExtractStatus::Failed | ExtractStatus::Unsupported) {
            state.report.failed += 1;
            continue;
        }

        state.report.extracted += 1;
        let discovered = extract.discovered_links.clone();
        let page_identity = item.request_url.to_string();
        let transaction = PageTransaction {
            page_identity: page_identity.clone(),
            extract,
            discovered_links: discovered.clone(),
        };
        match commit_page(transaction) {
            Ok(committed_identity) => {
                state.report.committed += 1;
                state.report.events.push(CrawlEvent::Committed {
                    page_identity: committed_identity.clone(),
                });
                committed_pages.push(committed_identity);
            }
            Err(error) => {
                state.report.failed += 1;
                state.report.events.push(CrawlEvent::Failed {
                    url: item.request_url.clone(),
                    reason: error.to_string(),
                });
                continue;
            }
        }

        for next in frontier_from_links(
            &mut state.queue,
            &item,
            &discovered,
            request.budget.max_depth,
        ) {
            state.report.events.push(CrawlEvent::Discovered {
                url: next.request_url.clone(),
                depth: next.depth,
            });
        }
    }

    Ok(CrawlOutput {
        report: state.report,
        committed_pages,
        artifacts,
    })
}

fn record_skipped(state: &mut CrawlState, item: &FrontierItem, reason: &str) {
    state.report.skipped += 1;
    state.report.events.push(CrawlEvent::Skipped {
        url: item.request_url.clone(),
        reason: reason.into(),
    });
}

#[cfg(test)]
mod tests {
    use crate::fetch::{MemoryEntry, MemoryTransport};

    use super::*;
    use crate::{HtmlExtractor, run_extract};

    #[test]
    fn crawls_linked_html_pages_offline() {
        let mut transport = MemoryTransport::new();
        transport.insert(
            "https://example.com/",
            MemoryEntry {
                status: 200,
                headers: BTreeMap::from([("content-type".into(), "text/html".into())]),
                body: br#"<html><head><title>Home</title></head><body><a href="/next">next</a> hello</body></html>"#.to_vec(),
            },
        );
        transport.insert(
            "https://example.com/next",
            MemoryEntry {
                status: 200,
                headers: BTreeMap::from([("content-type".into(), "text/html".into())]),
                body: br#"<html><head><title>Next</title></head><body>next page</body></html>"#.to_vec(),
            },
        );

        let request = CrawlRequest::from_seed("https://example.com/").expect("seed");
        let html = HtmlExtractor;
        let extractors: [&dyn Extractor; 1] = [&html];
        let output = run_crawl(request, transport, &extractors).expect("crawl");
        assert_eq!(output.report.committed, 2);
        assert_eq!(output.committed_pages.len(), 2);
    }

    #[test]
    fn extract_mode_reuses_artifact_without_transport() {
        let artifact = crate::fetch::build_artifact(
            url::Url::parse("https://example.com/page").expect("url"),
            url::Url::parse("https://example.com/page").expect("url"),
            200,
            BTreeMap::from([("content-type".into(), "text/html".into())]),
            pandark_types::BodyStorage::inline(
                b"<html><head><title>T</title></head><body>body</body></html>",
            ),
            Vec::new(),
            pandark_types::TransportProvenance::Test,
            None,
        );
        let html = HtmlExtractor;
        let extractors: [&dyn Extractor; 1] = [&html];
        let result = run_extract(
            &artifact,
            &extractors,
            ExtractBudget::default(),
            &ExtractContext::default(),
        );
        assert_eq!(result.status, ExtractStatus::Complete);
        assert!(result.document.is_some());
    }
}
