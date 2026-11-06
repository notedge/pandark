// gate: G-ARCH-4
// fixture: arch.wasm.capability_fixture
//
// The adapter must complete `fetch -> extract -> report` with nothing but
// host-injected capabilities. No local file, no embedded YYDB and no browser
// process is available here: the fixture supplies a memory fetch, a memory
// store, a fixed clock and an explicit cancellation flag.

use std::collections::{BTreeMap, HashMap};

use crawler_types::{CrawlBudgetStatus, TransportProvenance, diagnostics};
use pandark_wasm::{
    CancelCapability, Capabilities, CapabilityError, ClockCapability, FetchCapability, FetchResponse, RunRequest,
    StoreCapability, run_with_capabilities,
};

struct MemoryFetch {
    responses: HashMap<String, FetchResponse>,
    calls: std::cell::RefCell<Vec<String>>,
}

impl MemoryFetch {
    fn new(pages: &[(&str, &str)]) -> Self {
        let responses = pages
            .iter()
            .map(|(url, body)| {
                (
                    (*url).to_string(),
                    FetchResponse {
                        final_url: (*url).to_string(),
                        status: 200,
                        content_type: Some("text/html".into()),
                        headers: BTreeMap::from([("content-type".to_string(), "text/html".to_string())]),
                        body: body.as_bytes().to_vec(),
                        provenance: TransportProvenance::Test,
                    },
                )
            })
            .collect();
        Self { responses, calls: std::cell::RefCell::new(Vec::new()) }
    }

    fn call_count(&self) -> usize {
        self.calls.borrow().len()
    }
}

impl FetchCapability for MemoryFetch {
    fn fetch(&self, url: &str) -> Result<FetchResponse, CapabilityError> {
        self.calls.borrow_mut().push(url.to_string());
        self.responses
            .get(url)
            .cloned()
            .ok_or_else(|| CapabilityError::new("fetch", format!("fixture has no response for `{url}`")))
    }
}

#[derive(Default)]
struct MemoryStore {
    entries: HashMap<String, Vec<u8>>,
    fail_writes: bool,
}

impl StoreCapability for MemoryStore {
    fn put(&mut self, key: &str, payload: &[u8]) -> Result<(), CapabilityError> {
        if self.fail_writes {
            return Err(CapabilityError::new("store", "fixture store rejected the write"));
        }
        self.entries.insert(key.to_string(), payload.to_vec());
        Ok(())
    }
}

struct FixedClock(u64);

impl ClockCapability for FixedClock {
    fn now_epoch(&self) -> u64 {
        self.0
    }
}

struct NeverCancel;

impl CancelCapability for NeverCancel {
    fn is_cancelled(&self) -> bool {
        false
    }
}

struct AlwaysCancel;

impl CancelCapability for AlwaysCancel {
    fn is_cancelled(&self) -> bool {
        true
    }
}

fn run(
    request: &RunRequest,
    fetch: &dyn FetchCapability,
    store: &mut dyn StoreCapability,
    cancel: &dyn CancelCapability,
) -> Result<pandark_wasm::RunOutcome, CapabilityError> {
    let clock = FixedClock(1_700_000_000);
    let mut capabilities = Capabilities { fetch, store, clock: &clock, cancel };
    run_with_capabilities(request, &mut capabilities)
}

const PAGE_HTML: &str = r#"<!doctype html>
<html lang="en">
  <head><title>Gate page</title></head>
  <body>
    <h1>Capability fixture</h1>
    <p>The WASM adapter extracted this page without a native runtime.</p>
    <a href="https://example.com/next">next</a>
  </body>
</html>
"#;

#[test]
fn g_arch_wasm_capability_fixture() {
    let seed = "https://example.com/";
    let fetch = MemoryFetch::new(&[(seed, PAGE_HTML)]);
    let mut store = MemoryStore::default();
    let outcome = run(&RunRequest::new([seed], 4), &fetch, &mut store, &NeverCancel).expect("fixture run must succeed");

    assert_eq!(fetch.call_count(), 1, "the run must fetch exactly the seeded page");
    assert_eq!(outcome.report.schema_version, diagnostics::SCHEMA_REPORT, "the report envelope must use the Crawler schema");
    assert_eq!(outcome.report.extracted, 1, "the HTML page must extract");
    assert_eq!(outcome.report.committed, 1, "the extracted document must reach the store");
    assert_eq!(outcome.report.failed, 0);
    assert_eq!(outcome.report.budget_status, CrawlBudgetStatus::Completed);
    assert_eq!(outcome.pages.len(), 1);
    assert_eq!(outcome.pages[0].final_url, seed);

    let page_key = format!("pandark-wasm/page/{seed}");
    let stored = store.entries.get(&page_key).expect("the document payload must be stored under the page key");
    assert!(!stored.is_empty(), "the stored document payload must not be empty");
    assert!(
        String::from_utf8_lossy(stored).contains("Capability fixture"),
        "the stored document must keep the extracted heading text"
    );
    let report_key = format!("pandark-wasm/report/{seed}");
    assert!(store.entries.contains_key(&report_key), "the report envelope must reach the store");
    assert!(outcome.report_json.contains(diagnostics::SCHEMA_REPORT));
}

#[test]
fn g_arch_wasm_rejects_empty_seed_set() {
    let fetch = MemoryFetch::new(&[]);
    let mut store = MemoryStore::default();
    let error = run(&RunRequest::new(Vec::<String>::new(), 4), &fetch, &mut store, &NeverCancel)
        .expect_err("an empty seed set must be rejected");
    assert_eq!(error.capability(), "request", "the failure must be attributed to the request capability");
}

#[test]
fn g_arch_wasm_honours_cancellation() {
    let seed = "https://example.com/";
    let fetch = MemoryFetch::new(&[(seed, PAGE_HTML)]);
    let mut store = MemoryStore::default();
    let outcome = run(&RunRequest::new([seed], 4), &fetch, &mut store, &AlwaysCancel).expect("a cancelled run is not an error");
    assert_eq!(outcome.report.budget_status, CrawlBudgetStatus::Cancelled);
    assert_eq!(fetch.call_count(), 0, "a cancelled run must not fetch");
}

#[test]
fn g_arch_wasm_store_failure_is_structured() {
    let seed = "https://example.com/";
    let fetch = MemoryFetch::new(&[(seed, PAGE_HTML)]);
    let mut store = MemoryStore { fail_writes: true, ..Default::default() };
    let error = run(&RunRequest::new([seed], 4), &fetch, &mut store, &NeverCancel)
        .expect_err("a rejected store write must not be silently dropped");
    assert_eq!(error.capability(), "store");
    assert!(!error.message().is_empty());
}
