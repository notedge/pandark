//! Pandark store contract and backends.

#![deny(missing_docs)]

mod error;
pub mod keyspace;
mod memory;
mod store;
mod types;

pub use error::{Result, StoreError};
pub use memory::{frontier_item_from_contract, MemoryStore};
pub use store::Store;
pub use types::{
    ClaimToken, PagePhase, PageTxHandle, RunHandle, RunSpec, StoreConfig,
};
