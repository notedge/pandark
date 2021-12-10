#![warn(missing_docs)]
#![doc = include_str!("../readme.md")]

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
    if request.seeds.is_empty() {
        return Err(CrawlError::InvalidInput("at least one seed is required".into()));
    }

    let frontier = request
        .seeds
        .iter()
        .map(|seed| seed.url.to_string())
        .collect();

    Ok(CrawlPlan {
        request,
        frontier,
    })
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

#[cfg(test)]
mod tests {
    use pandark_types::CrawlRequest;

    use super::{initial_report, plan_crawl};

    #[test]
    fn plan_from_seed() {
        let request = CrawlRequest::from_seed("https://example.com").expect("seed");
        let plan = plan_crawl(request).expect("plan");
        assert_eq!(plan.frontier, vec!["https://example.com/"]);
    }

    #[test]
    fn empty_report_uses_primary_seed() {
        let request = CrawlRequest::from_seed("https://example.com/docs").expect("seed");
        let report = initial_report(&request).expect("report");
        assert_eq!(report.schema_version, "pandark.report/v1");
        assert_eq!(report.seed.as_str(), "https://example.com/docs");
    }
}
