Pandark web crawler framework
=============================

> **Status (2026-10-02):** architecture skeleton accepted. **Not** a production crawler yet — treat as an **offline/fixture prototype** until the four acceptance gates in the design doc pass (Oak HTML, robots/sitemap fixtures, HTTP semantics fixtures, multi-host politeness fixtures).

Pandark collects and extracts web pages into `notedown-ir::DocumentGraph`. It is **not** Panduck: Panduck converts between document formats on local files, while Pandark crawls URLs and builds semantic documents from fetched content.

## Crates

- `pandark-types` — crawl contracts, `FetchArtifact`, frontier items, reports, and policies
- `pandark-fetch` — HTTP, file, and memory transports with optional response cache
- `pandark-extract` — semantic extractors over `FetchArtifact`
- `pandark-store` — crawl persistence (`MemoryStore`, optional `YydbStore`)
- `pandark` — crawl orchestration Rust facade
- `pandark-napi` — Node-API bindings (`plan_crawl`, `crawl_file`, extract, inspect)
- `pandark-wasm` — WASM extract, inspect, and plan helpers (no crawl)

Browser sessions use a `BrowserProvider` capability layer on the native path only.

## Developers

```bash
cargo test
cargo build --release
pnpm install
pnpm run build:napi
pnpm --filter @notedge/pandark test:e2e
```
