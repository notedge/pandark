#![warn(missing_docs)]
#![doc = include_str!("../readme.md")]

mod admission;
mod crawl;
mod extract;
mod frontier;
mod transaction;

pub use admission::{admit_frontier_item, seed_frontier};
pub use crawl::{CrawlOutput, CrawlState, run_crawl};
pub use extract::{Extractor, HtmlExtractor, run_extract, select_extractor};
pub use frontier::{FrontierQueue, frontier_from_links};
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
