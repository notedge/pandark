use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{CrawlError, Result};

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
        let parsed = Url::parse(url).map_err(|error| CrawlError::InvalidInput(error.to_string()))?;
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
        Self {
            max_requests: 500,
            max_depth: 2,
        }
    }
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
        Ok(Self {
            seeds: vec![CrawlSeed::parse(url)?],
            politeness: PolitenessProfile::Conservative,
            budget: CrawlBudget::default(),
        })
    }
}
