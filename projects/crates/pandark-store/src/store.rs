use pandark_types::FrontierItem;

use crate::error::Result;
use crate::types::{ClaimToken, PagePhase, PageTxHandle, RunHandle, RunSpec, StoreConfig};

/// Store contract implemented by `MemoryStore` and `YydbStore`.
pub trait Store {
    /// Open or create a store backend.
    fn open(config: StoreConfig) -> Result<Self>
    where
        Self: Sized;

    /// Close and reopen the backend, simulating process restart.
    fn reopen(&mut self) -> Result<()>;

    /// Current store schema version.
    fn schema_version(&self) -> u32;

    /// Start a crawl run.
    fn begin_run(&mut self, spec: RunSpec) -> Result<RunHandle>;

    /// Enqueue a frontier item.
    fn enqueue(&mut self, item: FrontierItem) -> Result<()>;

    /// Claim the next queued item for a host partition.
    fn claim_next(&mut self, host_key: &str, worker_id: &str) -> Result<Option<ClaimToken>>;

    /// Attempt a competing claim for dual-worker contract cases.
    fn second_worker_claim(&mut self, worker_id: &str) -> Result<()>;

    /// Begin a page transaction bound to the active claim.
    fn begin_page_tx(&mut self, worker_id: &str) -> Result<PageTxHandle>;

    /// Attach a fetch artifact fixture to the active page transaction.
    fn page_tx_put_fetch(&mut self, fixture: &str, bytes: &[u8]) -> Result<()>;

    /// Attach an IR package fixture to the active page transaction.
    fn page_tx_put_ir(&mut self, fixture: &str, bytes: &[u8]) -> Result<()>;

    /// Commit the active page transaction.
    fn commit_page_tx(&mut self) -> Result<()>;

    /// Abort the active page transaction without committing.
    fn abort_page_tx(&mut self) -> Result<()>;

    /// Number of report events appended in the active run.
    fn event_count(&self) -> Result<u64>;

    /// Read a page phase, or `auto` for the most recent page.
    fn page_phase(&self, page_id: &str) -> Result<PagePhase>;
}
