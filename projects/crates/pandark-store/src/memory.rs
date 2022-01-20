use pandark_types::{FrontierItem, site_key};
use url::Url;

use crate::error::{Result, StoreError};
use crate::session::StoreSession;
use crate::types::{ClaimToken, PagePhase, PageTxHandle, RunHandle, RunSpec, StoreConfig};
use crate::Store;

/// In-memory `Store` backend for deterministic contract tests.
#[derive(Debug, Clone)]
pub struct MemoryStore {
    session: StoreSession,
}

impl Store for MemoryStore {
    fn open(config: StoreConfig) -> Result<Self> {
        Ok(Self {
            session: StoreSession::new(config.schema_version),
        })
    }

    fn reopen(&mut self) -> Result<()> {
        self.session.reopen()
    }

    fn schema_version(&self) -> u32 {
        self.session.schema_version()
    }

    fn begin_run(&mut self, spec: RunSpec) -> Result<RunHandle> {
        self.session.begin_run(spec)
    }

    fn enqueue(&mut self, item: FrontierItem) -> Result<()> {
        self.session.enqueue(item)
    }

    fn claim_next(&mut self, host_key: &str, worker_id: &str) -> Result<Option<ClaimToken>> {
        self.session.claim_next(host_key, worker_id)
    }

    fn second_worker_claim(&mut self, worker_id: &str) -> Result<()> {
        self.session.second_worker_claim(worker_id)
    }

    fn begin_page_tx(&mut self, worker_id: &str) -> Result<PageTxHandle> {
        self.session.begin_page_tx(worker_id)
    }

    fn page_tx_put_fetch(&mut self, fixture: &str, bytes: &[u8]) -> Result<()> {
        self.session.page_tx_put_fetch(fixture, bytes)
    }

    fn page_tx_put_ir(&mut self, fixture: &str, bytes: &[u8]) -> Result<()> {
        self.session.page_tx_put_ir(fixture, bytes)
    }

    fn commit_page_tx(&mut self) -> Result<()> {
        self.session.commit_page_tx()
    }

    fn abort_page_tx(&mut self) -> Result<()> {
        self.session.abort_page_tx()
    }

    fn event_count(&self) -> Result<u64> {
        self.session.event_count()
    }

    fn page_phase(&self, page_id: &str) -> Result<PagePhase> {
        self.session.page_phase(page_id)
    }
}

/// Build a [`FrontierItem`] from contract JSON fields.
pub fn frontier_item_from_contract(
    canonical_request_url: &str,
    depth: u32,
) -> Result<FrontierItem> {
    let request_url = Url::parse(canonical_request_url)
        .map_err(|error| StoreError::StoreCorrupt(error.to_string()))?;
    let site = site_key(&request_url);
    Ok(FrontierItem {
        request_url,
        discovery: pandark_types::DiscoverySource::Seed,
        depth,
        site_key: site,
        priority: 0,
        attempts: 0,
        discovered_at_epoch: pandark_types::now_epoch(),
    })
}
