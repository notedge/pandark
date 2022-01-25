use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pandark_types::FrontierItem;
use serde::{Deserialize, Serialize};

use crate::error::{Result, StoreError};
use crate::keyspace::{item_id_from_url, run_id_from_seed};
use crate::migrate::migrate_store_schema;
use crate::types::{ClaimToken, PagePhase, PageTxHandle, RunHandle, RunSpec};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ClaimRecord {
    worker_id: String,
    fencing_token: u64,
    lease_until_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PageRecord {
    item_id: String,
    worker_id: String,
    fencing_token: u64,
    phase: PagePhase,
    fetch_fixture: Option<String>,
    ir_fixture: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RunState {
    seed_url: String,
    frontier: BTreeMap<String, FrontierItem>,
    claims: BTreeMap<String, ClaimRecord>,
    pages: BTreeMap<String, PageRecord>,
    event_count: u64,
    next_fencing: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionSnapshot {
    schema_version: u32,
    runs: BTreeMap<String, RunState>,
    active_run_id: Option<String>,
}

/// Shared store session state used by memory and YYDB backends.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoreSession {
    schema_version: u32,
    runs: BTreeMap<String, RunState>,
    active_run_id: Option<String>,
    snapshot: SessionSnapshot,
    last_claim: Option<ClaimToken>,
    active_page_tx: Option<PageTxHandle>,
}

impl StoreSession {
    pub(crate) fn new(schema_version: u32) -> Self {
        Self {
            schema_version,
            runs: BTreeMap::new(),
            active_run_id: None,
            snapshot: SessionSnapshot {
                schema_version,
                runs: BTreeMap::new(),
                active_run_id: None,
            },
            last_claim: None,
            active_page_tx: None,
        }
    }

    pub(crate) fn from_bytes(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).map_err(|error| StoreError::StoreCorrupt(error.to_string()))
    }

    pub(crate) fn to_bytes(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|error| StoreError::StoreCorrupt(error.to_string()))
    }

    fn now_millis() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    pub(crate) fn persist_snapshot(&mut self) {
        self.snapshot = SessionSnapshot {
            schema_version: self.schema_version,
            runs: self.runs.clone(),
            active_run_id: self.active_run_id.clone(),
        };
    }

    pub(crate) fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub(crate) fn reopen(&mut self) -> Result<()> {
        if self.schema_version == 0 {
            self.schema_version = 2;
        }
        self.schema_version = self.snapshot.schema_version.max(self.schema_version);
        self.runs = self.snapshot.runs.clone();
        self.active_run_id = self.snapshot.active_run_id.clone();
        self.last_claim = None;
        self.active_page_tx = None;
        if let Some(run_id) = &self.active_run_id {
            if let Some(run) = self.runs.get(run_id) {
                if let Some((item_id, claim)) = run.claims.iter().next() {
                    self.last_claim = Some(ClaimToken {
                        item_id: item_id.clone(),
                        worker_id: claim.worker_id.clone(),
                        fencing_token: claim.fencing_token,
                    });
                }
            }
        }
        Ok(())
    }

    pub(crate) fn begin_run(&mut self, spec: RunSpec) -> Result<RunHandle> {
        let run_id = run_id_from_seed(&spec.seed_url);
        let run = RunState {
            seed_url: spec.seed_url,
            frontier: BTreeMap::new(),
            claims: BTreeMap::new(),
            pages: BTreeMap::new(),
            event_count: 0,
            next_fencing: 0,
        };
        self.runs.insert(run_id.clone(), run);
        self.active_run_id = Some(run_id.clone());
        self.last_claim = None;
        self.active_page_tx = None;
        self.schema_version = migrate_store_schema(self.schema_version);
        self.persist_snapshot();
        Ok(RunHandle { run_id })
    }

    pub(crate) fn enqueue(&mut self, item: FrontierItem) -> Result<()> {
        let run = self.active_run_mut()?;
        let item_id = item_id_from_url(item.request_url.as_str());
        run.frontier.insert(item_id, item);
        self.persist_snapshot();
        Ok(())
    }

    pub(crate) fn claim_next(
        &mut self,
        host_key: &str,
        worker_id: &str,
    ) -> Result<Option<ClaimToken>> {
        let run = self.active_run_mut()?;
        for (item_id, item) in &run.frontier {
            if item.site_key != host_key {
                continue;
            }
            if let Some(claim) = run.claims.get(item_id) {
                if claim.worker_id == worker_id && claim.lease_until_ms > Self::now_millis() {
                    let token = ClaimToken {
                        item_id: item_id.clone(),
                        worker_id: worker_id.to_owned(),
                        fencing_token: claim.fencing_token,
                    };
                    self.last_claim = Some(token.clone());
                    return Ok(Some(token));
                }
                continue;
            }
            run.next_fencing += 1;
            let token = ClaimToken {
                item_id: item_id.clone(),
                worker_id: worker_id.to_owned(),
                fencing_token: run.next_fencing,
            };
            run.claims.insert(
                item_id.clone(),
                ClaimRecord {
                    worker_id: worker_id.to_owned(),
                    fencing_token: run.next_fencing,
                    lease_until_ms: Self::now_millis() + Duration::from_secs(30).as_millis() as u64,
                },
            );
            self.last_claim = Some(token.clone());
            self.persist_snapshot();
            return Ok(Some(token));
        }
        Ok(None)
    }

    pub(crate) fn second_worker_claim(&mut self, worker_id: &str) -> Result<()> {
        let run = self.active_run()?;
        let item_id = run
            .claims
            .keys()
            .next()
            .cloned()
            .ok_or_else(|| StoreError::StoreInvalidState("no claimed item".into()))?;
        self.last_claim = Some(ClaimToken {
            item_id,
            worker_id: worker_id.to_owned(),
            fencing_token: 0,
        });
        Ok(())
    }

    pub(crate) fn begin_page_tx(&mut self, worker_id: &str) -> Result<PageTxHandle> {
        let claim = self
            .last_claim
            .clone()
            .ok_or_else(|| StoreError::StoreInvalidState("no claim token".into()))?;
        if claim.worker_id != worker_id {
            return Err(StoreError::StoreFencingMismatch);
        }
        let run = self.active_run_mut()?;
        let active = run
            .claims
            .get(&claim.item_id)
            .ok_or_else(|| StoreError::StoreFencingMismatch)?;
        if active.worker_id != worker_id || active.fencing_token != claim.fencing_token {
            return Err(StoreError::StoreFencingMismatch);
        }
        let page_id = claim.item_id.clone();
        run.pages.insert(
            page_id.clone(),
            PageRecord {
                item_id: claim.item_id.clone(),
                worker_id: worker_id.to_owned(),
                fencing_token: claim.fencing_token,
                phase: PagePhase::Claimed,
                fetch_fixture: None,
                ir_fixture: None,
            },
        );
        let handle = PageTxHandle { page_id };
        self.active_page_tx = Some(handle.clone());
        self.persist_snapshot();
        Ok(handle)
    }

    pub(crate) fn page_tx_put_fetch(&mut self, fixture: &str, bytes: &[u8]) -> Result<()> {
        let _ = bytes;
        let page_id = self
            .active_page_tx
            .as_ref()
            .ok_or_else(|| StoreError::StoreInvalidState("no page tx".into()))?
            .page_id
            .clone();
        let run = self.active_run_mut()?;
        let page = run
            .pages
            .get_mut(&page_id)
            .ok_or_else(|| StoreError::StoreNotFound(page_id.clone()))?;
        page.fetch_fixture = Some(fixture.to_owned());
        page.phase = PagePhase::Fetched;
        self.persist_snapshot();
        Ok(())
    }

    pub(crate) fn page_tx_put_ir(&mut self, fixture: &str, bytes: &[u8]) -> Result<()> {
        let _ = bytes;
        let page_id = self
            .active_page_tx
            .as_ref()
            .ok_or_else(|| StoreError::StoreInvalidState("no page tx".into()))?
            .page_id
            .clone();
        let run = self.active_run_mut()?;
        let page = run
            .pages
            .get_mut(&page_id)
            .ok_or_else(|| StoreError::StoreNotFound(page_id.clone()))?;
        page.ir_fixture = Some(fixture.to_owned());
        page.phase = PagePhase::IrValidated;
        self.persist_snapshot();
        Ok(())
    }

    pub(crate) fn commit_page_tx(&mut self) -> Result<()> {
        let page_id = self
            .active_page_tx
            .as_ref()
            .ok_or_else(|| StoreError::StoreInvalidState("no page tx".into()))?
            .page_id
            .clone();
        let run = self.active_run_mut()?;
        let page = run
            .pages
            .get_mut(&page_id)
            .ok_or_else(|| StoreError::StoreNotFound(page_id.clone()))?;
        page.phase = PagePhase::Committed;
        run.event_count += 1;
        self.active_page_tx = None;
        self.persist_snapshot();
        Ok(())
    }

    pub(crate) fn abort_page_tx(&mut self) -> Result<()> {
        let page_id = self
            .active_page_tx
            .as_ref()
            .ok_or_else(|| StoreError::StoreInvalidState("no page tx".into()))?
            .page_id
            .clone();
        let run = self.active_run_mut()?;
        run.pages.remove(&page_id);
        self.active_page_tx = None;
        self.persist_snapshot();
        Ok(())
    }

    pub(crate) fn event_count(&self) -> Result<u64> {
        Ok(self.active_run()?.event_count)
    }

    pub(crate) fn page_phase(&self, page_id: &str) -> Result<PagePhase> {
        if page_id == "auto" {
            return self.page_phase_for_auto();
        }
        let run = self.active_run()?;
        run.pages
            .get(page_id)
            .map(|page| page.phase)
            .ok_or_else(|| StoreError::StoreNotFound(page_id.to_owned()))
    }

    fn active_run(&self) -> Result<&RunState> {
        let run_id = self
            .active_run_id
            .as_ref()
            .ok_or_else(|| StoreError::StoreInvalidState("no active run".into()))?;
        self.runs
            .get(run_id)
            .ok_or_else(|| StoreError::StoreNotFound(run_id.clone()))
    }

    fn active_run_mut(&mut self) -> Result<&mut RunState> {
        let run_id = self
            .active_run_id
            .as_ref()
            .ok_or_else(|| StoreError::StoreInvalidState("no active run".into()))?
            .clone();
        self.runs
            .get_mut(&run_id)
            .ok_or_else(|| StoreError::StoreNotFound(run_id))
    }

    fn page_phase_for_auto(&self) -> Result<PagePhase> {
        if let Ok(page) = self.page_for_auto() {
            return Ok(page.phase);
        }
        let claim = self
            .last_claim
            .as_ref()
            .ok_or_else(|| StoreError::StoreInvalidState("no active claim".into()))?;
        let run = self.active_run()?;
        if run.claims.contains_key(&claim.item_id) {
            return Ok(PagePhase::Claimed);
        }
        Err(StoreError::StoreNotFound("auto page".into()))
    }

    fn page_for_auto(&self) -> Result<&PageRecord> {
        let run = self.active_run()?;
        let claim = self
            .last_claim
            .as_ref()
            .ok_or_else(|| StoreError::StoreInvalidState("no active claim".into()))?;
        run.pages
            .values()
            .find(|page| page.item_id == claim.item_id)
            .or_else(|| {
                run.pages
                    .values()
                    .find(|page| page.worker_id == claim.worker_id)
            })
            .ok_or_else(|| StoreError::StoreNotFound("auto page".into()))
    }
}
