//! WebAssembly adapter for Pandark.
//!
//! The adapter keeps native assumptions out of the portable core: network,
//! persistence, time and cancellation are injected by the host
//! ([`capability`]) and the crawl loop only composes `fetch -> extract ->
//! report` ([`run_with_capabilities`]).
//!
//! The crate builds for `wasm32-unknown-unknown` and the `G-ARCH-4` gate
//! asserts that the linked artifact imports no filesystem, browser process or
//! embedded database symbols.

#![deny(missing_docs)]

pub mod capability;
pub mod run;

#[cfg(target_arch = "wasm32")]
mod wasm_api;

pub use capability::{
    CancelCapability, Capabilities, CapabilityError, ClockCapability, FetchCapability, FetchResponse, StoreCapability,
};
pub use run::{PageOutcome, RunOutcome, RunRequest, StoredPageKind, run_with_capabilities};
