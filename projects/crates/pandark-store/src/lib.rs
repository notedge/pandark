//! Pandark store contract and backends.

#![deny(missing_docs)]

mod error;
pub mod keyspace;
pub mod migrate;
mod memory;
#[cfg(feature = "yydb")]
mod response_cache;
mod session;
mod store;
mod types;

#[cfg(feature = "yydb")]
mod yydb;

pub use error::{Result, StoreError};
pub use memory::{frontier_item_from_contract, MemoryStore};
pub use store::Store;
pub use types::{
    ClaimToken, PagePhase, PageTxHandle, RunHandle, RunSpec, StoreConfig, StoreDiagnosticReport,
    StoreDoctorIssue, StoreDoctorSeverity,
};

#[cfg(feature = "yydb")]
pub use response_cache::YydbResponseCache;
#[cfg(feature = "yydb")]
pub use yydb::YydbStore;
