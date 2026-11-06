# pandark-wasm

WebAssembly capability adapter for Pandark.

The adapter owns no native runtime. Network access, persistence, time and
cancellation are injected by the host through trait objects, and the crate
compiles for `wasm32-unknown-unknown` without importing a filesystem, a
browser process or an embedded database. The same capability core is exercised
natively by the `g_arch_wasm_capability_fixture` gate.
