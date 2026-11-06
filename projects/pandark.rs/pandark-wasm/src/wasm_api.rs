//! Minimal `wasm-bindgen` surface for the adapter.
//!
//! Capability objects cross the JS boundary through the TypeScript loader; the
//! Rust side only publishes its identity so a host can verify the loaded
//! artifact matches the package version.

use wasm_bindgen::prelude::*;

/// Returns the Pandark WASM adapter version.
#[wasm_bindgen]
pub fn pandark_wasm_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Returns the capability names the host must inject, as a JSON array.
#[wasm_bindgen]
pub fn pandark_wasm_capabilities_json() -> String {
    r#"["fetch","store","clock","cancel"]"#.to_string()
}
