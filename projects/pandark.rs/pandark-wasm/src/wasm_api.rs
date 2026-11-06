//! Plain `extern "C"` WebAssembly ABI for the capability adapter.
//!
//! The boundary deliberately has no bindgen glue and no imports. The host
//! writes two UTF-8 JSON documents into linear memory, calls
//! `pandark_wasm_run`, and reads back one JSON envelope. Network pages, clock
//! and cancellation arrive inside the capability document, and store writes
//! are returned to the host instead of being persisted by the module.

use std::{collections::BTreeMap, sync::Mutex};

use crawler_types::TransportProvenance;
use serde::{Deserialize, Serialize};

use crate::{
    PageOutcome,
    capability::{
        CancelCapability, Capabilities, CapabilityError, ClockCapability, FetchCapability, FetchResponse, StoreCapability,
    },
    run::{RunRequest, run_with_capabilities},
};

/// ABI version exposed to the host.
const ABI_VERSION: u32 = 1;

static RESULT: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static VERSION: &[u8] = env!("CARGO_PKG_VERSION").as_bytes();

/// One page snapshot supplied by the host fixture.
#[derive(Debug, Deserialize)]
struct FixturePage {
    url: String,
    #[serde(rename = "finalUrl")]
    final_url: Option<String>,
    status: u16,
    #[serde(rename = "contentType")]
    content_type: Option<String>,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    body: String,
    #[serde(default)]
    provenance: Option<String>,
}

/// Capability document supplied by the host.
#[derive(Debug, Deserialize)]
struct FixtureCapabilities {
    #[serde(default)]
    pages: Vec<FixturePage>,
    #[serde(default)]
    clock: u64,
    #[serde(default)]
    cancel: bool,
}

/// One store write returned to the host.
#[derive(Debug, Serialize)]
struct StoredRecord {
    key: String,
    payload: String,
}

/// Structured failure returned to the host.
#[derive(Debug, Serialize)]
struct Failure {
    capability: String,
    message: String,
}

/// Result envelope returned to the host.
#[derive(Debug, Serialize)]
struct Envelope {
    ok: bool,
    #[serde(rename = "reportJson", skip_serializing_if = "Option::is_none")]
    report_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pages: Option<Vec<PageOutcome>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stored: Option<Vec<StoredRecord>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Failure>,
}

struct FixtureFetch {
    pages: BTreeMap<String, FetchResponse>,
}

impl FetchCapability for FixtureFetch {
    fn fetch(&self, url: &str) -> Result<FetchResponse, CapabilityError> {
        self.pages
            .get(url)
            .cloned()
            .ok_or_else(|| CapabilityError::new("fetch", format!("no host page was injected for `{url}`")))
    }
}

struct CaptureStore {
    records: Vec<StoredRecord>,
}

impl StoreCapability for CaptureStore {
    fn put(&mut self, key: &str, payload: &[u8]) -> Result<(), CapabilityError> {
        self.records.push(StoredRecord { key: key.to_string(), payload: String::from_utf8_lossy(payload).into_owned() });
        Ok(())
    }
}

struct FixtureClock(u64);

impl ClockCapability for FixtureClock {
    fn now_epoch(&self) -> u64 {
        self.0
    }
}

struct FixtureCancel(bool);

impl CancelCapability for FixtureCancel {
    fn is_cancelled(&self) -> bool {
        self.0
    }
}

fn response_from(page: FixturePage) -> (String, FetchResponse) {
    let provenance = match page.provenance.as_deref() {
        Some("network") => TransportProvenance::Network,
        _ => TransportProvenance::Test,
    };
    let final_url = page.final_url.unwrap_or_else(|| page.url.clone());
    let response = FetchResponse {
        final_url,
        status: page.status,
        content_type: page.content_type,
        headers: page.headers,
        body: page.body.into_bytes(),
        provenance,
    };
    (page.url, response)
}

fn run(request: &[u8], capabilities: &[u8]) -> Envelope {
    let request: RunRequest = match serde_json::from_slice(request) {
        Ok(request) => request,
        Err(error) => {
            return Envelope {
                ok: false,
                report_json: None,
                pages: None,
                stored: None,
                error: Some(Failure { capability: "request".into(), message: error.to_string() }),
            };
        }
    };
    let capabilities: FixtureCapabilities = match serde_json::from_slice(capabilities) {
        Ok(capabilities) => capabilities,
        Err(error) => {
            return Envelope {
                ok: false,
                report_json: None,
                pages: None,
                stored: None,
                error: Some(Failure { capability: "capability".into(), message: error.to_string() }),
            };
        }
    };

    let pages: BTreeMap<String, FetchResponse> = capabilities.pages.into_iter().map(response_from).collect();
    let fetch = FixtureFetch { pages };
    let clock = FixtureClock(capabilities.clock);
    let cancel = FixtureCancel(capabilities.cancel);
    let mut store = CaptureStore { records: Vec::new() };
    let mut injected = Capabilities { fetch: &fetch, store: &mut store, clock: &clock, cancel: &cancel };

    match run_with_capabilities(&request, &mut injected) {
        Ok(outcome) => Envelope {
            ok: true,
            report_json: Some(outcome.report_json),
            pages: Some(outcome.pages),
            stored: Some(store.records),
            error: None,
        },
        Err(error) => Envelope {
            ok: false,
            report_json: None,
            pages: None,
            stored: None,
            error: Some(Failure { capability: error.capability().into(), message: error.message().into() }),
        },
    }
}

fn publish(envelope: &Envelope) -> i32 {
    let encoded = match serde_json::to_vec(envelope) {
        Ok(encoded) => encoded,
        Err(_) => return 1,
    };
    let mut result = RESULT.lock().unwrap_or_else(|poison| poison.into_inner());
    *result = encoded;
    0
}

/// Build a UTF-8 buffer that the host fills before calling the adapter.
#[unsafe(no_mangle)]
pub extern "C" fn pandark_wasm_alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return std::ptr::null_mut();
    }
    let mut bytes = Vec::<u8>::with_capacity(len);
    bytes.resize(len, 0);
    let pointer = bytes.as_mut_ptr();
    std::mem::forget(bytes);
    pointer
}

/// Release a buffer previously returned by [`pandark_wasm_alloc`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pandark_wasm_dealloc(pointer: *mut u8, len: usize) {
    if pointer.is_null() || len == 0 {
        return;
    }
    unsafe { drop(Vec::from_raw_parts(pointer, 0, len)) };
}

/// Run `fetch -> extract -> report` with the host-supplied capability document.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pandark_wasm_run(
    request_pointer: *const u8,
    request_len: usize,
    capabilities_pointer: *const u8,
    capabilities_len: usize,
) -> i32 {
    let request = if request_len == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(request_pointer, request_len) } };
    let capabilities = if capabilities_len == 0 {
        &[][..]
    }
    else {
        unsafe { std::slice::from_raw_parts(capabilities_pointer, capabilities_len) }
    };
    publish(&run(request, capabilities))
}

/// ABI version understood by the host loader.
#[unsafe(no_mangle)]
pub extern "C" fn pandark_wasm_abi_version() -> u32 {
    ABI_VERSION
}

/// Pointer to the adapter version string.
#[unsafe(no_mangle)]
pub extern "C" fn pandark_wasm_version_ptr() -> *const u8 {
    VERSION.as_ptr()
}

/// Length of the adapter version string.
#[unsafe(no_mangle)]
pub extern "C" fn pandark_wasm_version_len() -> usize {
    VERSION.len()
}

/// Pointer to the last result envelope.
#[unsafe(no_mangle)]
pub extern "C" fn pandark_wasm_result_ptr() -> *const u8 {
    RESULT.lock().unwrap_or_else(|poison| poison.into_inner()).as_ptr()
}

/// Length of the last result envelope.
#[unsafe(no_mangle)]
pub extern "C" fn pandark_wasm_result_len() -> usize {
    RESULT.lock().unwrap_or_else(|poison| poison.into_inner()).len()
}
