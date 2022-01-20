//! Pandark store contract and backends.

#![deny(missing_docs)]

mod error;
pub mod keyspace;
mod memory;
mod session;
mod store;
mod types;

#[cfg(feature = "yydb")]
mod yydb;

pub use error::{Result, StoreError};
pub use memory::{frontier_item_from_contract, MemoryStore};
pub use store::Store;
pub use types::{
    ClaimToken, PagePhase, PageTxHandle, RunHandle, RunSpec, StoreConfig,
};

#[cfg(feature = "yydb")]
pub use yydb::YydbStore;
