use boost_diagnostic::{DiagnosticSeverity, Message, MessageArg};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    diagnostics as pd,
    error::{CrawlError, Result},
};

/// Politeness preset for crawl scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolitenessProfile {
    /// Conservative defaults: robots, low concurrency, backoff.
    Conservative,
    /// Balanced throughput within host limits.
    Balanced,
    /// Developer mode with minimal throttling.
    Developer,
}

/// A crawl entry point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrawlSeed {
    /// Canonical seed URL.
    pub url: Url,
}

impl CrawlSeed {
    /// Parse a seed URL string.
    pub fn parse(url: &str) -> Result<Self> {
        let parsed = Url::parse(url).map_err(|error| {
            let diag = boost_diagnostic::Diagnostic::new(
                pd::code(pd::URL_INVALID),
                DiagnosticSeverity::Error,
                pd::origin_fetch(),
                Message::new("pandark.diagnostic.seedUrl")
                    .with_arg("input", MessageArg::Text(url.to_string()))
                    .with_arg("error", MessageArg::Text(error.to_string()))
                    .with_fallback(format!("invalid seed URL `{url}`: {error}")),
            );
            CrawlError::from(diag)
        })?;
        Ok(Self { url: parsed })
    }
}

/// Hard limits for a crawl run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrawlBudget {
    /// Maximum HTTP requests including assets.
    pub max_requests: u32,
    /// Maximum link depth from seeds.
    pub max_depth: u32,
}

impl Default for CrawlBudget {
    fn default() -> Self {
        Self { max_requests: 500, max_depth: 2 }
    }
}

/// When to invoke a configured browser provider during crawl.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BrowserFallbackPolicy {
    /// Never invoke browser fallback.
    #[default]
    Never,
    /// Capture a browser snapshot when HTTP fetch fails.
    OnFetchFailure,
}

/// User-facing crawl request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrawlRequest {
    /// One or more seeds.
    pub seeds: Vec<CrawlSeed>,
    /// Scheduling politeness preset.
    pub politeness: PolitenessProfile,
    /// Hard crawl limits.
    pub budget: CrawlBudget,
}

impl CrawlRequest {
    /// Build a request from a single seed URL.
    pub fn from_seed(url: &str) -> Result<Self> {
        Self::from_seeds([url])
    }

    /// Build a request from multiple seed URL strings.
    pub fn from_seeds(urls: impl IntoIterator<Item = impl AsRef<str>>) -> Result<Self> {
        let seeds = urls.into_iter().map(|url| CrawlSeed::parse(url.as_ref())).collect::<Result<Vec<_>>>()?;
        if seeds.is_empty() {
            let diag = boost_diagnostic::Diagnostic::new(
                pd::code(pd::REQUEST_INVALID),
                DiagnosticSeverity::Error,
                pd::origin_fetch(),
                Message::new("pandark.diagnostic.seedsEmpty")
                    .with_fallback("at least one seed URL is required to build a CrawlRequest"),
            );
            return Err(CrawlError::from(diag));
        }
        Ok(Self { seeds, politeness: PolitenessProfile::Conservative, budget: CrawlBudget::default() })
    }
}
