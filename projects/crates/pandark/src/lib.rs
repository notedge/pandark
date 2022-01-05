#![warn(missing_docs)]
#![doc = include_str!("../readme.md")]

mod admission;
mod browser;
mod crawl;
mod extract;
mod fetch;
mod frontier;
mod input;
mod inspect;
mod napi;
mod transaction;

pub use admission::{admit_frontier_item, seed_frontier};
pub use browser::{
    BrowserProvider, BrowserProviderStack, ChallengeBlocked, FixtureBrowserProvider,
    HttpBrowserProvider, build_browser_stack, load_fixture_provider_from_dir,
    load_fixture_provider_from_dirs, load_snapshot_json, parse_browser_dirs,
    path_looks_like_snapshot, run_extract_from_snapshot, save_snapshot_json,
    snapshot_to_fetch_artifact,
};
pub use crawl::{CrawlOptions, CrawlOutput, CrawlState, resume_crawl_from_checkpoint, run_crawl, run_crawl_with_options};
pub use extract::{Extractor, HtmlExtractor, run_extract, select_extractor};
pub use fetch::{
    FetchClient, FetchRequest, FileTransport, HttpTransport, MemoryEntry, MemoryTransport,
    ResponseCache, RobotsPolicy, SeedTransport, Transport, build_artifact, fetch_url,
    map_transport_error, transport_for_url,
};
pub use frontier::{FrontierQueue, frontier_from_links};
pub use input::{InputFormat, LoadedInput, load_input, parse_input_format};
pub use inspect::{inspect_artifact, inspect_snapshot};
pub use transaction::commit_page;

use pandark_types::{CrawlError, CrawlReport, CrawlRequest, Result};

/// Planned crawl work units without performing network I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrawlPlan {
    /// Normalized request used to build the plan.
    pub request: CrawlRequest,
    /// Seed URLs expanded for the frontier.
    pub frontier: Vec<String>,
}

/// Build a crawl plan from a request.
pub fn plan_crawl(request: CrawlRequest) -> Result<CrawlPlan> {
    let items = seed_frontier(&request)?;
    let frontier = items
        .iter()
        .map(|item| item.request_url.to_string())
        .collect();
    Ok(CrawlPlan { request, frontier })
}

/// Initialize an empty report for the primary seed.
pub fn initial_report(request: &CrawlRequest) -> Result<CrawlReport> {
    let seed = request
        .seeds
        .first()
        .ok_or_else(|| CrawlError::InvalidInput("at least one seed is required".into()))?
        .url
        .clone();
    Ok(CrawlReport::empty(seed))
}
