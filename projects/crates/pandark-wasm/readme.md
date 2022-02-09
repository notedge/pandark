# pandark-wasm

WebAssembly adapter for Pandark extract, inspect, and plan helpers. Targets browser and Worker hosts that need semantic
HTML processing without a Node native binary.

## ✅ Exported in WASM

- Extract and inspect over in-memory or bundled inputs
- Crawl **planning** previews (frontier sizing without full network crawl)

## 🚫 Not exported

Crawl orchestration is **intentionally absent** from WASM. Hosts cannot safely run a full multi-request crawler from a
web page context with the same contracts as Node.

## 🔌 npm packaging

There is no separate `@notedge/pandark-wasm` npm carrier in this release. WASM builds integrate where browser adapters
are wired; Node users should use `@notedge/pandark` native bindings.

Build with `wasm-pack` from the repository when developing the adapter crate directly.

License: MPL-2.0
