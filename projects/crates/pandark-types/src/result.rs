use notedown_ir::DocumentGraph;
use serde::{Deserialize, Serialize};
use url::Url;

/// Outcome of a single fetch operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FetchOutcome {
    /// Final response URL after redirects.
    pub final_url: Url,
    /// Response media type when known.
    pub content_type: Option<String>,
    /// Raw body bytes.
    pub body: Vec<u8>,
}

/// Outcome of semantic extraction into Notedown IR.
#[derive(Debug, Clone)]
pub struct ExtractOutcome {
    /// Source page URL.
    pub source_url: Url,
    /// Extracted semantic document.
    pub document: DocumentGraph,
}

/// Per-page crawl status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrawlPageStatus {
    /// Fetch and extraction succeeded.
    Extracted,
    /// Fetch succeeded but extraction failed or was skipped.
    FetchOnly,
    /// Page failed to fetch or extract.
    Failed,
    /// Skipped by budget, robots, or policy.
    Skipped,
}

/// High-level crawl report aligned with `pandark.report/v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrawlReport {
    /// Report schema identifier.
    pub schema_version: String,
    /// Primary seed URL.
    pub seed: Url,
    /// Pages extracted successfully.
    pub extracted: u32,
    /// Pages that failed.
    pub failed: u32,
    /// Pages skipped by policy or budget.
    pub skipped: u32,
}

impl CrawlReport {
    /// Empty report for a seed.
    pub fn empty(seed: Url) -> Self {
        Self {
            schema_version: "pandark.report/v1".to_string(),
            seed,
            extracted: 0,
            failed: 0,
            skipped: 0,
        }
    }
}
