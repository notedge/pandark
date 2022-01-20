use serde::{Deserialize, Serialize};

/// Store open parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreConfig {
    /// Expected store schema version.
    pub schema_version: u32,
    /// File path for file-backed backends such as `YydbStore`.
    pub db_path: Option<std::path::PathBuf>,
}

impl StoreConfig {
    /// Memory-oriented defaults.
    pub fn memory(schema_version: u32) -> Self {
        Self {
            schema_version,
            db_path: None,
        }
    }

    /// YYDB file path for integration tests and production.
    pub fn yydb(path: impl Into<std::path::PathBuf>, schema_version: u32) -> Self {
        Self {
            schema_version,
            db_path: Some(path.into()),
        }
    }
}

/// Parameters for starting a crawl run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSpec {
    /// Seed URL for the run.
    pub seed_url: String,
}

/// Active run identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunHandle {
    /// Stable run identifier.
    pub run_id: String,
}

/// Lease token issued when a worker claims a frontier item.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ClaimToken {
    /// Frontier item identifier.
    pub item_id: String,
    /// Worker that holds the claim.
    pub worker_id: String,
    /// Monotonic fencing token.
    pub fencing_token: u64,
}

/// In-flight page transaction handle.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PageTxHandle {
    /// Page record identifier.
    pub page_id: String,
}

/// Page transaction lifecycle phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PagePhase {
    /// Item claimed but no page transaction started.
    Claimed,
    /// Fetch artifact stored.
    Fetched,
    /// IR package stored.
    IrValidated,
    /// Page transaction committed.
    Committed,
}

impl PagePhase {
    /// Stable contract string used by store contract assertions.
    pub fn as_contract_str(self) -> &'static str {
        match self {
            Self::Claimed => "claimed",
            Self::Fetched => "fetched",
            Self::IrValidated => "ir_validated",
            Self::Committed => "committed",
        }
    }
}
