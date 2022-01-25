//! Data-driven store contract runner for `MemoryStore` and `YydbStore`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use pandark_store::{
    frontier_item_from_contract, MemoryStore, Store, StoreConfig, StoreDiagnosticReport,
    StoreDoctorSeverity, StoreError,
};
use serde::Deserialize;

#[cfg(feature = "yydb")]
use pandark_store::YydbStore;

const CONTRACT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/store_contract");

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(rename = "case")]
    cases: Vec<ManifestCase>,
}

#[derive(Debug, Deserialize)]
struct ManifestCase {
    id: String,
    gate: String,
    file: String,
    backends: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CaseFile {
    case_id: String,
    initial: CaseSection,
    steps: Vec<CaseStep>,
    assertions: Vec<CaseAssertion>,
}

#[derive(Debug, Deserialize)]
struct CaseSection {
    steps: Vec<CaseStep>,
}

#[derive(Debug, Deserialize)]
struct CaseStep {
    op: String,
    #[serde(default)]
    run_spec: Option<RunSpecJson>,
    #[serde(default)]
    frontier_item: Option<FrontierItemJson>,
    #[serde(default)]
    host_key: Option<String>,
    #[serde(default)]
    worker_id: Option<String>,
    #[serde(default)]
    fixture: Option<String>,
    #[serde(default)]
    store_schema_version: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct RunSpecJson {
    seed_url: String,
}

#[derive(Debug, Deserialize)]
struct FrontierItemJson {
    canonical_request_url: String,
    depth: u32,
}

#[derive(Debug, Deserialize)]
struct CaseAssertion {
    kind: String,
    #[serde(default)]
    page_id: Option<String>,
    #[serde(default)]
    expected: Option<String>,
    #[serde(default)]
    min: Option<u64>,
    #[serde(default)]
    max: Option<u64>,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    severity: Option<String>,
}

pub fn run_memory_suite() {
    run_suite("memory");
}

#[cfg(feature = "yydb")]
pub fn run_yydb_suite() {
    run_suite("yydb");
}

#[cfg(feature = "yydb")]
pub fn run_yydb_doctor_suite() {
    run_suite_filtered("yydb", |case| case.id == "store.doctor.quota_orphan");
}

pub fn run_migration_suite(backend: &str) {
    run_suite_filtered(backend, |case| case.id == "store.migration.v1_to_v2");
}

fn run_suite(backend: &str) {
    run_suite_filtered(backend, |_| true);
}

fn run_suite_filtered(backend: &str, filter: impl Fn(&ManifestCase) -> bool) {
    let manifest_path = Path::new(CONTRACT_DIR).join("manifest.toml");
    let manifest_text = fs::read_to_string(manifest_path).expect("manifest.toml");
    let manifest: Manifest = toml::from_str(&manifest_text).expect("parse manifest");
    for case in manifest.cases {
        if !case.backends.iter().any(|item| item == backend) {
            continue;
        }
        if !filter(&case) {
            continue;
        }
        let case_path = Path::new(CONTRACT_DIR).join(&case.file);
        match backend {
            "memory" => run_case_memory(&case.gate, &case.id, &case_path),
            #[cfg(feature = "yydb")]
            "yydb" => run_case_yydb(&case.gate, &case.id, &case_path),
            other => panic!("unsupported backend {other}"),
        }
    }
}

fn run_case_memory(gate: &str, case_id: &str, case_path: &Path) {
    let case_file = load_case(case_id, case_path);
    let mut store = MemoryStore::open(StoreConfig::memory(1)).expect("open memory store");
    let mut last_error = None;
    let mut last_doctor_report = None;
    for step in &case_file.initial.steps {
        if step.op == "open" {
            let schema_version = step.store_schema_version.unwrap_or(1);
            store = MemoryStore::open(StoreConfig::memory(schema_version)).expect("open memory store");
            continue;
        }
        last_error = execute_step(&mut store, step, &mut last_doctor_report).err();
    }
    for step in &case_file.steps {
        last_error = execute_step(&mut store, step, &mut last_doctor_report).err();
    }
    evaluate_assertions(
        gate,
        case_id,
        &store,
        &case_file.assertions,
        last_error,
        last_doctor_report,
    );
}

#[cfg(feature = "yydb")]
fn run_case_yydb(gate: &str, case_id: &str, case_path: &Path) {
    let case_file = load_case(case_id, case_path);
    let db_path = temp_db_path(case_id);
    cleanup_db(&db_path);
    let mut store =
        YydbStore::open(StoreConfig::yydb(&db_path, 1)).expect("open yydb store");
    let mut last_error = None;
    let mut last_doctor_report = None;
    for step in &case_file.initial.steps {
        if step.op == "open" {
            cleanup_db(&db_path);
            let schema_version = step.store_schema_version.unwrap_or(1);
            store = YydbStore::open(StoreConfig::yydb(&db_path, schema_version))
                .expect("open yydb store");
            continue;
        }
        last_error = execute_step(&mut store, step, &mut last_doctor_report).err();
    }
    for step in &case_file.steps {
        last_error = execute_step(&mut store, step, &mut last_doctor_report).err();
    }
    evaluate_assertions(
        gate,
        case_id,
        &store,
        &case_file.assertions,
        last_error,
        last_doctor_report,
    );
    cleanup_db(&db_path);
}

fn load_case(case_id: &str, case_path: &Path) -> CaseFile {
    let text = fs::read_to_string(case_path).expect("read case json");
    let case_file: CaseFile = serde_json::from_str(&text).expect("parse case json");
    assert_eq!(
        case_file.case_id,
        case_id,
        "case_id mismatch in {}",
        case_path.display()
    );
    case_file
}

fn execute_step<S: Store>(
    store: &mut S,
    step: &CaseStep,
    last_doctor_report: &mut Option<StoreDiagnosticReport>,
) -> Result<(), StoreError> {
    match step.op.as_str() {
        "begin_run" => {
            let spec = step.run_spec.as_ref().expect("begin_run run_spec");
            store
                .begin_run(pandark_store::RunSpec {
                    seed_url: spec.seed_url.clone(),
                })
                .map(|_| ())
        }
        "enqueue" => {
            let item = step.frontier_item.as_ref().expect("enqueue frontier_item");
            let frontier = frontier_item_from_contract(
                &item.canonical_request_url,
                item.depth,
            )?;
            store.enqueue(frontier)
        }
        "claim_next" => {
            let host_key = step.host_key.as_ref().expect("claim_next host_key");
            let worker_id = step.worker_id.as_ref().expect("claim_next worker_id");
            store.claim_next(host_key, worker_id).map(|_| ())
        }
        "second_worker_claim" => {
            let worker_id = step.worker_id.as_ref().expect("second_worker_claim worker_id");
            store.second_worker_claim(worker_id)
        }
        "begin_page_tx" => {
            let worker_id = step.worker_id.as_ref().expect("begin_page_tx worker_id");
            store.begin_page_tx(worker_id).map(|_| ())
        }
        "page_tx_put_fetch" => {
            let fixture = step.fixture.as_ref().expect("page_tx_put_fetch fixture");
            let bytes = read_fixture(fixture)?;
            store.page_tx_put_fetch(fixture, &bytes)
        }
        "page_tx_put_ir" => {
            let fixture = step.fixture.as_ref().expect("page_tx_put_ir fixture");
            let bytes = read_fixture(fixture)?;
            store.page_tx_put_ir(fixture, &bytes)
        }
        "commit_page_tx" => store.commit_page_tx(),
        "abort_page_tx" => store.abort_page_tx(),
        "reopen" => store.reopen(),
        "put_orphan_object" => {
            let bytes = if let Some(fixture) = &step.fixture {
                read_fixture(fixture)?
            } else {
                b"orphan-contract-payload".to_vec()
            };
            store.put_orphan_object(&bytes)
        }
        "doctor" => {
            *last_doctor_report = Some(store.doctor()?);
            Ok(())
        }
        other => Err(StoreError::StoreInvalidState(format!("unsupported op {other}"))),
    }
}

fn read_fixture(name: &str) -> Result<Vec<u8>, StoreError> {
    let path = Path::new(CONTRACT_DIR).join("fixtures").join(name);
    fs::read(path).map_err(|error| StoreError::StoreCorrupt(error.to_string()))
}

fn evaluate_assertions<S: Store>(
    gate: &str,
    case_id: &str,
    store: &S,
    assertions: &[CaseAssertion],
    last_error: Option<StoreError>,
    last_doctor_report: Option<StoreDiagnosticReport>,
) {
    for assertion in assertions {
        match assertion.kind.as_str() {
            "page_phase" => {
                let page_id = assertion.page_id.as_deref().unwrap_or("auto");
                let expected = assertion.expected.as_deref().expect("page_phase expected");
                let actual = store
                    .page_phase(page_id)
                    .expect("page_phase query")
                    .as_contract_str();
                assert_eq!(
                    actual,
                    expected,
                    "gate {gate} case {case_id} page_phase expected {expected}, got {actual}"
                );
            }
            "event_count" => {
                let count = store.event_count().expect("event_count");
                let min = assertion.min.unwrap_or(0);
                let max = assertion.max.unwrap_or(u64::MAX);
                assert!(
                    count >= min && count <= max,
                    "gate {gate} case {case_id} event_count {count} not in [{min}, {max}]"
                );
            }
            "schema_version" => {
                let expected = assertion
                    .expected
                    .as_deref()
                    .expect("schema_version expected");
                let actual = store.schema_version().to_string();
                assert_eq!(
                    actual,
                    expected,
                    "gate {gate} case {case_id} schema_version expected {expected}, got {actual}"
                );
            }
            "error" => {
                let code = assertion.code.as_deref().expect("error code");
                let err = last_error.as_ref().unwrap_or_else(|| {
                    panic!("gate {gate} case {case_id} expected error {code}, got success")
                });
                let actual = format!("{err}");
                assert!(
                    actual.contains(code),
                    "gate {gate} case {case_id} expected error {code}, got {actual}"
                );
            }
            "doctor_issue" => {
                let code = assertion.code.as_deref().expect("doctor_issue code");
                let severity = assertion
                    .severity
                    .as_deref()
                    .expect("doctor_issue severity");
                let expected_severity =
                    StoreDoctorSeverity::parse(severity).expect("doctor_issue severity");
                let report = last_doctor_report.as_ref().unwrap_or_else(|| {
                    panic!("gate {gate} case {case_id} expected doctor report for {code}")
                });
                let issue = report
                    .issues
                    .iter()
                    .find(|issue| issue.code == code)
                    .unwrap_or_else(|| {
                        panic!(
                            "gate {gate} case {case_id} expected doctor issue {code}, got {:?}",
                            report.issues
                        )
                    });
                assert_eq!(
                    issue.severity.as_contract_str(),
                    expected_severity.as_contract_str(),
                    "gate {gate} case {case_id} doctor issue {code} severity mismatch"
                );
            }
            other => panic!("gate {gate} case {case_id} unsupported assertion kind {other}"),
        }
    }
}

#[cfg(feature = "yydb")]
fn temp_db_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("pandark-store-{label}-{nonce}.yydb"))
}

#[cfg(feature = "yydb")]
fn cleanup_db(path: &Path) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(format!("{}-wal", path.display()));
    let _ = fs::remove_file(format!("{}-shm", path.display()));
    let _ = fs::remove_dir_all(format!("{}.objects", path.display()));
}
