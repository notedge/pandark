use std::path::{Path, PathBuf};

use yydb::{Connection, ObjectKind};

use crate::error::{Result, StoreError};
use crate::keyspace;
use crate::session::StoreSession;
use crate::types::{
    ClaimToken, PagePhase, PageTxHandle, RunHandle, RunSpec, StoreConfig, StoreDiagnosticReport,
    StoreDoctorIssue, StoreDoctorSeverity,
};
use crate::Store;
use pandark_types::FrontierItem;

const SESSION_KEY: &str = "__pandark/store/session";

/// File-backed store using embedded `yydb::Connection`.
pub struct YydbStore {
    path: PathBuf,
    conn: Connection,
    session: StoreSession,
}

impl YydbStore {
    fn load_session(conn: &Connection) -> Result<Option<StoreSession>> {
        match conn.get(SESSION_KEY).map_err(map_yydb_error)? {
            Some(bytes) => Ok(Some(StoreSession::from_bytes(&bytes)?)),
            None => Ok(None),
        }
    }

    fn persist(&mut self) -> Result<()> {
        self.conn
            .put(SESSION_KEY, &self.session.to_bytes()?)
            .map_err(map_yydb_error)?;
        self.conn
            .put(
                keyspace::schema_version(),
                &self.session.schema_version().to_le_bytes(),
            )
            .map_err(map_yydb_error)
    }
}

impl Store for YydbStore {
    fn open(config: StoreConfig) -> Result<Self> {
        let path = config
            .db_path
            .clone()
            .ok_or_else(|| StoreError::StoreInvalidState("YydbStore requires db_path".into()))?;
        let conn = Connection::open(&path).map_err(map_yydb_error)?;
        let session = Self::load_session(&conn)?.unwrap_or_else(|| StoreSession::new(config.schema_version));
        Ok(Self {
            path,
            conn,
            session,
        })
    }

    fn reopen(&mut self) -> Result<()> {
        self.conn = Connection::open(&self.path).map_err(map_yydb_error)?;
        if let Some(bytes) = self.conn.get(SESSION_KEY).map_err(map_yydb_error)? {
            self.session = StoreSession::from_bytes(&bytes)?;
        }
        self.session.reopen()?;
        self.persist()?;
        Ok(())
    }

    fn schema_version(&self) -> u32 {
        self.session.schema_version()
    }

    fn begin_run(&mut self, spec: RunSpec) -> Result<RunHandle> {
        let handle = self.session.begin_run(spec)?;
        self.persist()?;
        Ok(handle)
    }

    fn enqueue(&mut self, item: FrontierItem) -> Result<()> {
        self.session.enqueue(item)?;
        self.persist()?;
        Ok(())
    }

    fn claim_next(&mut self, host_key: &str, worker_id: &str) -> Result<Option<ClaimToken>> {
        let token = self.session.claim_next(host_key, worker_id)?;
        self.persist()?;
        Ok(token)
    }

    fn second_worker_claim(&mut self, worker_id: &str) -> Result<()> {
        self.session.second_worker_claim(worker_id)
    }

    fn begin_page_tx(&mut self, worker_id: &str) -> Result<PageTxHandle> {
        let handle = self.session.begin_page_tx(worker_id)?;
        self.persist()?;
        Ok(handle)
    }

    fn page_tx_put_fetch(&mut self, fixture: &str, bytes: &[u8]) -> Result<()> {
        self.session.page_tx_put_fetch(fixture, bytes)?;
        self.persist()?;
        Ok(())
    }

    fn page_tx_put_ir(&mut self, fixture: &str, bytes: &[u8]) -> Result<()> {
        self.session.page_tx_put_ir(fixture, bytes)?;
        self.persist()?;
        Ok(())
    }

    fn commit_page_tx(&mut self) -> Result<()> {
        self.session.commit_page_tx()?;
        self.persist()?;
        Ok(())
    }

    fn abort_page_tx(&mut self) -> Result<()> {
        self.session.abort_page_tx()?;
        self.persist()?;
        Ok(())
    }

    fn event_count(&self) -> Result<u64> {
        self.session.event_count()
    }

    fn page_phase(&self, page_id: &str) -> Result<PagePhase> {
        self.session.page_phase(page_id)
    }

    fn put_orphan_object(&mut self, bytes: &[u8]) -> Result<()> {
        self.conn
            .put_chunk(ObjectKind::Blob, bytes)
            .map_err(map_yydb_error)?;
        Ok(())
    }

    fn doctor(&self) -> Result<StoreDiagnosticReport> {
        let report = self.conn.doctor().map_err(map_yydb_error)?;
        let mut issues = Vec::new();
        if report.orphan_object_count > 0 {
            issues.push(StoreDoctorIssue {
                severity: StoreDoctorSeverity::Warn,
                code: "store.doctor.orphan_object".into(),
                message: format!(
                    "{} orphan objects are not referenced by committed metadata",
                    report.orphan_object_count
                ),
            });
        }
        Ok(StoreDiagnosticReport {
            orphan_object_count: report.orphan_object_count,
            issues,
        })
    }
}

impl YydbStore {
    /// Database path for diagnostics.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn map_yydb_error(error: yydb::Error) -> StoreError {
    StoreError::StoreCorrupt(error.to_string())
}
