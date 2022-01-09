use std::collections::BTreeMap;

use crate::browser::{BrowserProvider, snapshot_to_fetch_artifact};
use crate::fetch::{FetchClient, FetchRequest, RobotsPolicy, Transport};
use pandark_types::{
    AdmissionDecision, apply_challenge_policy, BrowserFallbackPolicy, ChallengeOutcome,
    ChallengePolicy, CrawlCheckpoint, CrawlEvent, CrawlReport, CrawlRequest, ExtractBudget,
    ExtractContext, ExtractStatus, FetchArtifact, FrontierItem, PageChallengeState, Result,
};

use crate::admission::{admit_frontier_item, seed_frontier};
use crate::extract::{Extractor, run_extract};
use crate::frontier::{FrontierQueue, frontier_from_links};
use crate::initial_report;
use crate::transaction::commit_page;
use pandark_types::PageTransaction;

/// Optional browser and fallback settings for a crawl run.
#[derive(Clone, Copy)]
pub struct CrawlOptions<'a> {
    /// Browser provider used when fallback policy allows it.
    pub browser: Option<&'a dyn BrowserProvider>,
    /// When to invoke the browser provider.
    pub browser_fallback: BrowserFallbackPolicy,
    /// Challenge handling for browser snapshots.
    pub challenge_policy: ChallengePolicy,
    /// Optional persistent fetch cache directory.
    pub cache_dir: Option<&'a std::path::Path>,
    /// Strategy fingerprint recorded into checkpoints.
    pub strategy_fingerprint: Option<&'a str>,
    /// Attempt identity incremented on resume.
    pub attempt_identity: u32,
}

impl<'a> Default for CrawlOptions<'a> {
    fn default() -> Self {
        Self {
            browser: None,
            browser_fallback: BrowserFallbackPolicy::Never,
            challenge_policy: ChallengePolicy::Stop,
            cache_dir: None,
            strategy_fingerprint: None,
            attempt_identity: 0,
        }
    }
}

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
    /// Last frontier item paused for operator action.
    pub last_paused_item: Option<FrontierItem>,
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
    /// Checkpoint when the run paused for operator action.
    pub checkpoint: Option<CrawlCheckpoint>,
}

/// Run a bounded crawl using the provided transport.
pub fn run_crawl<T: Transport>(
    request: CrawlRequest,
    transport: T,
    extractors: &[&dyn Extractor],
) -> Result<CrawlOutput> {
    run_crawl_with_options(request, transport, extractors, CrawlOptions::default())
}

/// Run a bounded crawl with optional browser fallback.
pub fn run_crawl_with_options<T: Transport>(
    request: CrawlRequest,
    transport: T,
    extractors: &[&dyn Extractor],
    options: CrawlOptions<'_>,
) -> Result<CrawlOutput> {
    let mut state = CrawlState {
        request: request.clone(),
        queue: FrontierQueue::new(),
        requests_used: 0,
        report: initial_report(&request)?,
        last_paused_item: None,
    };

    for item in seed_frontier(&request)? {
        state.report.events.push(CrawlEvent::Discovered {
            url: item.request_url.clone(),
            depth: item.depth,
        });
        state.queue.push(item);
    }

    run_crawl_loop(state, transport, extractors, options)
}

/// Resume a crawl from a saved checkpoint.
pub fn resume_crawl_from_checkpoint<T: Transport>(
    checkpoint: CrawlCheckpoint,
    transport: T,
    extractors: &[&dyn Extractor],
    options: CrawlOptions<'_>,
) -> Result<CrawlOutput> {
    let mut state = CrawlState {
        request: checkpoint.request,
        queue: FrontierQueue::new(),
        requests_used: checkpoint.requests_used,
        report: checkpoint.report,
        last_paused_item: checkpoint.paused_item.clone(),
    };
    state.queue.restore(checkpoint.pending_frontier);
    if let Some(item) = checkpoint.paused_item {
        state.queue.requeue(item);
    }
    let resume_options = CrawlOptions {
        attempt_identity: checkpoint.attempt_identity.saturating_add(1),
        ..options
    };
    run_crawl_loop(state, transport, extractors, resume_options)
}

fn run_crawl_loop<T: Transport>(
    mut state: CrawlState,
    transport: T,
    extractors: &[&dyn Extractor],
    options: CrawlOptions<'_>,
) -> Result<CrawlOutput> {
    let request = state.request.clone();
    let mut client = FetchClient::new(transport, RobotsPolicy::from_profile(request.politeness));
    if let Some(cache_dir) = options.cache_dir {
        client = client.with_disk_cache_dir(cache_dir)?;
    }
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
                let fetch_error = error.to_string();
                match try_browser_fallback(&item, options) {
                    Some((snapshot, browser_engine)) => {
                        state.report.events.push(CrawlEvent::BrowserFallback {
                            url: item.request_url.clone(),
                            fetch_error,
                            browser_engine,
                        });
                        match resolve_browser_snapshot(snapshot, options.challenge_policy) {
                            BrowserPageAction::Proceed(artifact) => artifact,
                            BrowserPageAction::Skip(reason) => {
                                record_skipped(&mut state, &item, &reason);
                                continue;
                            }
                            BrowserPageAction::Pause(challenge_state) => {
                                record_paused(&mut state, &item, challenge_state);
                                continue;
                            }
                            BrowserPageAction::Fail(reason) => {
                                state.report.failed += 1;
                                state.report.events.push(CrawlEvent::Failed {
                                    url: item.request_url.clone(),
                                    reason,
                                });
                                continue;
                            }
                        }
                    }
                    None => {
                        state.report.failed += 1;
                        state.report.events.push(CrawlEvent::Failed {
                            url: item.request_url.clone(),
                            reason: fetch_error,
                        });
                        continue;
                    }
                }
            }
        };

        process_fetched_item(
            &mut state,
            &item,
            artifact,
            extractors,
            extract_budget,
            &mut committed_pages,
            &mut artifacts,
            request.budget.max_depth,
        );
    }

    let checkpoint = if state.last_paused_item.is_some() {
        Some(build_checkpoint(&state, options))
    } else {
        None
    };

    Ok(CrawlOutput {
        report: state.report,
        committed_pages,
        artifacts,
        checkpoint,
    })
}

fn try_browser_fallback(
    item: &FrontierItem,
    options: CrawlOptions<'_>,
) -> Option<(pandark_types::BrowserSnapshot, String)> {
    if options.browser_fallback != BrowserFallbackPolicy::OnFetchFailure {
        return None;
    }
    let browser = options.browser?;
    let snapshot = browser.capture_snapshot(&item.request_url).ok()?;
    let engine = snapshot.browser_engine.clone();
    Some((snapshot, engine))
}

enum BrowserPageAction {
    Proceed(FetchArtifact),
    Skip(String),
    Pause(PageChallengeState),
    Fail(String),
}

fn resolve_browser_snapshot(
    snapshot: pandark_types::BrowserSnapshot,
    policy: ChallengePolicy,
) -> BrowserPageAction {
    let challenge_state = snapshot.challenge_state;
    let outcome = apply_challenge_policy(challenge_state, policy);
    match outcome {
        ChallengeOutcome::Proceed => {
            BrowserPageAction::Proceed(snapshot_to_fetch_artifact(&snapshot))
        }
        ChallengeOutcome::Skip => {
            BrowserPageAction::Skip(challenge_reason("skipped", challenge_state))
        }
        ChallengeOutcome::PauseForOperator => BrowserPageAction::Pause(challenge_state),
        ChallengeOutcome::Stop => {
            BrowserPageAction::Fail(challenge_reason("blocked", challenge_state))
        }
        ChallengeOutcome::FallbackHttp => BrowserPageAction::Fail(
            "challenge-fallback-http-unavailable".into(),
        ),
    }
}

fn challenge_reason(prefix: &str, state: PageChallengeState) -> String {
    format!("{prefix}:{}", serde_json::to_string(&state).unwrap_or_else(|_| "unknown".into()))
}

fn process_fetched_item(
    state: &mut CrawlState,
    item: &FrontierItem,
    artifact: FetchArtifact,
    extractors: &[&dyn Extractor],
    extract_budget: ExtractBudget,
    committed_pages: &mut Vec<String>,
    artifacts: &mut Vec<FetchArtifact>,
    max_depth: u32,
) {
    let cache_hit = artifact.transport_provenance == pandark_types::TransportProvenance::CacheHit;
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
        return;
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
            if state
                .last_paused_item
                .as_ref()
                .is_some_and(|paused| paused.request_url == item.request_url)
            {
                state.last_paused_item = None;
            }
        }
        Err(error) => {
            state.report.failed += 1;
            state.report.events.push(CrawlEvent::Failed {
                url: item.request_url.clone(),
                reason: error.to_string(),
            });
            return;
        }
    }

    for next in frontier_from_links(&mut state.queue, item, &discovered, max_depth) {
        state.report.events.push(CrawlEvent::Discovered {
            url: next.request_url.clone(),
            depth: next.depth,
        });
    }
}

fn record_skipped(state: &mut CrawlState, item: &FrontierItem, reason: &str) {
    state.report.skipped += 1;
    state.report.events.push(CrawlEvent::Skipped {
        url: item.request_url.clone(),
        reason: reason.into(),
    });
}

fn record_paused(state: &mut CrawlState, item: &FrontierItem, challenge_state: PageChallengeState) {
    state.requests_used = state.requests_used.saturating_sub(1);
    state.report.paused += 1;
    state.last_paused_item = Some(item.clone());
    state.report.events.push(CrawlEvent::PausedForOperator {
        url: item.request_url.clone(),
        challenge_state,
    });
}

fn build_checkpoint(state: &CrawlState, options: CrawlOptions<'_>) -> CrawlCheckpoint {
    let (paused_url, paused_challenge) = last_pause_from_report(&state.report);
    CrawlCheckpoint::new(
        state.request.clone(),
        state.queue.pending_items().to_vec(),
        state.requests_used,
        state.report.clone(),
        paused_url,
        paused_challenge,
        state.last_paused_item.clone(),
        options
            .strategy_fingerprint
            .map(str::to_string)
            .or_else(|| Some(default_strategy_fingerprint(options))),
        options.attempt_identity,
    )
}

fn default_strategy_fingerprint(options: CrawlOptions<'_>) -> String {
    format!(
        "browser_fallback={:?};challenge_policy={:?}",
        options.browser_fallback, options.challenge_policy
    )
}

fn last_pause_from_report(report: &CrawlReport) -> (Option<url::Url>, Option<PageChallengeState>) {
    for event in report.events.events().iter().rev() {
        if let CrawlEvent::PausedForOperator {
            url,
            challenge_state,
        } = event
        {
            return (Some(url.clone()), Some(*challenge_state));
        }
    }
    (None, None)
}

#[cfg(test)]
mod tests {
    use crate::fetch::{MemoryEntry, MemoryTransport};

    use super::*;
    use crate::{FixtureBrowserProvider, HtmlExtractor, resume_crawl_from_checkpoint, run_extract};
    use pandark_types::{BrowserSnapshot, ChallengePolicy, PageChallengeState};
    use url::Url;

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
    fn browser_fallback_commits_when_http_fetch_missing() {
        let transport = MemoryTransport::new();
        let mut browser = FixtureBrowserProvider::new();
        browser.insert(BrowserSnapshot {
            requested_url: Url::parse("https://example.com/js-page").expect("url"),
            final_url: Url::parse("https://example.com/js-page").expect("url"),
            document_html:
                "<html><head><title>JS</title></head><body><p>rendered</p></body></html>"
                    .into(),
            captured_at_epoch: 1,
            browser_engine: "fixture".into(),
            profile_id: "test".into(),
            challenge_state: PageChallengeState::Normal,
        });

        let request = CrawlRequest::from_seed("https://example.com/js-page").expect("seed");
        let html = HtmlExtractor;
        let extractors: [&dyn Extractor; 1] = [&html];
        let output = run_crawl_with_options(
            request,
            transport,
            &extractors,
            CrawlOptions {
                browser: Some(&browser),
                browser_fallback: BrowserFallbackPolicy::OnFetchFailure,
                challenge_policy: ChallengePolicy::Stop,
                ..Default::default()
            },
        )
        .expect("crawl");
        assert_eq!(output.report.committed, 1);
        assert!(
            output
                .report
                .events
                .events()
                .iter()
                .any(|event| matches!(event, CrawlEvent::BrowserFallback { .. }))
        );
    }

    #[test]
    fn browser_fallback_skips_challenge_page_when_policy_allows() {
        let transport = MemoryTransport::new();
        let mut browser = FixtureBrowserProvider::new();
        browser.insert(BrowserSnapshot {
            requested_url: Url::parse("https://example.com/private").expect("url"),
            final_url: Url::parse("https://example.com/login").expect("url"),
            document_html: "<html><body>login</body></html>".into(),
            captured_at_epoch: 1,
            browser_engine: "fixture".into(),
            profile_id: "test".into(),
            challenge_state: PageChallengeState::LoginRequired,
        });

        let request = CrawlRequest::from_seed("https://example.com/private").expect("seed");
        let html = HtmlExtractor;
        let extractors: [&dyn Extractor; 1] = [&html];
        let output = run_crawl_with_options(
            request,
            transport,
            &extractors,
            CrawlOptions {
                browser: Some(&browser),
                browser_fallback: BrowserFallbackPolicy::OnFetchFailure,
                challenge_policy: ChallengePolicy::SkipPage,
                ..Default::default()
            },
        )
        .expect("crawl");
        assert_eq!(output.report.skipped, 1);
        assert_eq!(output.report.failed, 0);
        assert_eq!(output.report.committed, 0);
    }

    #[test]
    fn browser_fallback_fails_challenge_page_when_policy_stops() {
        let transport = MemoryTransport::new();
        let mut browser = FixtureBrowserProvider::new();
        browser.insert(BrowserSnapshot {
            requested_url: Url::parse("https://example.com/private").expect("url"),
            final_url: Url::parse("https://example.com/login").expect("url"),
            document_html: "<html><body>login</body></html>".into(),
            captured_at_epoch: 1,
            browser_engine: "fixture".into(),
            profile_id: "test".into(),
            challenge_state: PageChallengeState::LoginRequired,
        });

        let request = CrawlRequest::from_seed("https://example.com/private").expect("seed");
        let html = HtmlExtractor;
        let extractors: [&dyn Extractor; 1] = [&html];
        let output = run_crawl_with_options(
            request,
            transport,
            &extractors,
            CrawlOptions {
                browser: Some(&browser),
                browser_fallback: BrowserFallbackPolicy::OnFetchFailure,
                challenge_policy: ChallengePolicy::Stop,
                ..Default::default()
            },
        )
        .expect("crawl");
        assert_eq!(output.report.skipped, 0);
        assert_eq!(output.report.failed, 1);
        assert_eq!(output.report.committed, 0);
    }

    #[test]
    fn browser_fallback_pauses_and_resumes_after_operator_snapshot() {
        let seed = Url::parse("https://example.com/private").expect("url");
        let transport = MemoryTransport::new();
        let mut browser = FixtureBrowserProvider::new();
        browser.insert(BrowserSnapshot {
            requested_url: seed.clone(),
            final_url: Url::parse("https://example.com/login").expect("url"),
            document_html: "<html><body>login</body></html>".into(),
            captured_at_epoch: 1,
            browser_engine: "fixture".into(),
            profile_id: "test".into(),
            challenge_state: PageChallengeState::ChallengeRequired,
        });

        let request = CrawlRequest::from_seed(seed.as_str()).expect("seed");
        let html = HtmlExtractor;
        let extractors: [&dyn Extractor; 1] = [&html];
        let options = CrawlOptions {
            browser: Some(&browser),
            browser_fallback: BrowserFallbackPolicy::OnFetchFailure,
            challenge_policy: ChallengePolicy::PauseForOperator,
            ..Default::default()
        };
        let paused = run_crawl_with_options(request, transport, &extractors, options).expect("pause");
        assert_eq!(paused.report.paused, 1);
        let checkpoint = paused.checkpoint.expect("checkpoint");
        let strategy_fp = checkpoint.strategy_fingerprint.clone();
        let attempt_id = checkpoint.attempt_identity;

        browser.insert(BrowserSnapshot {
            requested_url: seed.clone(),
            final_url: seed.clone(),
            document_html:
                "<html><head><title>OK</title></head><body><p>ready</p></body></html>".into(),
            captured_at_epoch: 2,
            browser_engine: "fixture".into(),
            profile_id: "test".into(),
            challenge_state: PageChallengeState::Normal,
        });

        let resumed = resume_crawl_from_checkpoint(
            checkpoint,
            MemoryTransport::new(),
            &extractors,
            CrawlOptions {
                browser: Some(&browser),
                browser_fallback: BrowserFallbackPolicy::OnFetchFailure,
                challenge_policy: ChallengePolicy::Stop,
                ..Default::default()
            },
        )
        .expect("resume");
        assert_eq!(resumed.report.committed, 1);
        assert_eq!(resumed.report.paused, 1);
        assert_eq!(resumed.checkpoint, None);
        assert!(strategy_fp.is_some());
        assert_eq!(attempt_id, 0);
    }

    #[test]
    fn disk_cache_reuses_response_without_transport() {
        let dir = std::env::temp_dir().join(format!("pandark-crawl-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut transport = MemoryTransport::new();
        transport.insert(
            "https://example.com/cache-page",
            MemoryEntry {
                status: 200,
                headers: BTreeMap::from([("content-type".into(), "text/html".into())]),
                body: br#"<html><head><title>Cached</title></head><body>cached</body></html>"#
                    .to_vec(),
            },
        );
        let request = CrawlRequest::from_seed("https://example.com/cache-page").expect("seed");
        let html = HtmlExtractor;
        let extractors: [&dyn Extractor; 1] = [&html];
        let options = CrawlOptions {
            cache_dir: Some(dir.as_path()),
            ..Default::default()
        };
        let first = run_crawl_with_options(request.clone(), transport, &extractors, options)
            .expect("first crawl");
        assert_eq!(first.report.committed, 1);

        let second = run_crawl_with_options(
            request,
            MemoryTransport::new(),
            &extractors,
            CrawlOptions {
                cache_dir: Some(dir.as_path()),
                ..Default::default()
            },
        )
        .expect("second crawl");
        assert_eq!(second.report.committed, 1);
        assert!(
            second
                .report
                .events
                .events()
                .iter()
                .any(|event| matches!(
                    event,
                    CrawlEvent::Fetched { cache_hit: true, .. }
                ))
        );
        let _ = std::fs::remove_dir_all(&dir);
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
