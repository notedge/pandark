use boost_diagnostic::{DiagnosticEnvelope, DiagnosticSet};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::events::EventLog;

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

/// High-level crawl report aligned with `pandark.report/v2`.
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
    /// Pages paused awaiting operator action.
    pub paused: u32,
    /// Pages committed to persistent storage.
    pub committed: u32,
    /// Structured event log snapshot.
    pub events: EventLog,
    /// Structured diagnostic envelope for run-level errors and warnings.
    ///
    /// Schema version: `boost-diagnostic DiagnosticEnvelope`. Every
    /// `CrawlError::Diagnostic` emitted during the run is pushed here
    /// so CLI/N-API consumers can surface stable codes and typed args.
    #[serde(default = "default_diagnostic_envelope", skip_serializing_if = "is_envelope_empty")]
    pub diagnostics: DiagnosticEnvelope,
}

fn is_envelope_empty(env: &DiagnosticEnvelope) -> bool {
    env.diagnostics.is_empty()
}

fn default_diagnostic_envelope() -> DiagnosticEnvelope {
    DiagnosticEnvelope::from_set(&DiagnosticSet::new())
}

impl CrawlReport {
    /// Empty report for a seed.
    pub fn empty(seed: Url) -> Self {
        Self {
            schema_version: "pandark.report/v2".to_string(),
            seed,
            extracted: 0,
            failed: 0,
            skipped: 0,
            paused: 0,
            committed: 0,
            events: EventLog::new(),
            diagnostics: DiagnosticEnvelope::from_set(&DiagnosticSet::new()),
        }
    }

    /// Collect a run-level diagnostic into the report envelope.
    pub fn push_diagnostic(&mut self, diagnostic: boost_diagnostic::Diagnostic) {
        let mut set = DiagnosticSet::new();
        for d in &self.diagnostics.diagnostics {
            set.push(d.clone());
        }
        set.push(diagnostic);
        self.diagnostics = DiagnosticEnvelope::from_set(&set);
    }
}
