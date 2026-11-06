//! Host-injected capabilities for the WASM adapter.
//!
//! The adapter never reaches for a native runtime itself: every side effect is
//! supplied by the host through one of the traits below. A host that cannot
//! provide a capability must fail the run with a structured error instead of
//! silently falling back to a local file, an embedded database or a fake store.

use std::{collections::BTreeMap, error::Error, fmt};

use crawler_types::TransportProvenance;

/// Structured capability failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityError {
    capability: &'static str,
    message: String,
}

impl CapabilityError {
    /// Build an error for one named capability.
    pub fn new(capability: &'static str, message: impl Into<String>) -> Self {
        Self { capability, message: message.into() }
    }

    /// Capability that rejected the operation.
    pub fn capability(&self) -> &'static str {
        self.capability
    }

    /// Human-readable failure detail.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for CapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} capability failed: {}", self.capability, self.message)
    }
}

impl Error for CapabilityError {}

/// Response returned by the host fetch capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchResponse {
    /// Final URL after any host-side redirects.
    pub final_url: String,
    /// Final HTTP status.
    pub status: u16,
    /// Parsed media type when the host knows it.
    pub content_type: Option<String>,
    /// Response headers in wire order keys.
    pub headers: BTreeMap<String, String>,
    /// Inline response body bytes.
    pub body: Vec<u8>,
    /// How the host obtained the bytes.
    pub provenance: TransportProvenance,
}

/// Network capability supplied by the host.
pub trait FetchCapability {
    /// Fetch one URL. The host owns transport selection, redirects and budgets.
    fn fetch(&self, url: &str) -> Result<FetchResponse, CapabilityError>;
}

/// Persistence capability supplied by the host.
pub trait StoreCapability {
    /// Store one opaque payload under a stable key.
    fn put(&mut self, key: &str, payload: &[u8]) -> Result<(), CapabilityError>;
}

/// Clock capability supplied by the host.
pub trait ClockCapability {
    /// Current Unix epoch seconds.
    fn now_epoch(&self) -> u64;
}

/// Cancellation capability supplied by the host.
pub trait CancelCapability {
    /// Whether the host asked the run to stop.
    fn is_cancelled(&self) -> bool;
}

/// Capability set injected into one run.
pub struct Capabilities<'a> {
    /// Network access.
    pub fetch: &'a dyn FetchCapability,
    /// Host persistence.
    pub store: &'a mut dyn StoreCapability,
    /// Host clock.
    pub clock: &'a dyn ClockCapability,
    /// Host cancellation signal.
    pub cancel: &'a dyn CancelCapability,
}
