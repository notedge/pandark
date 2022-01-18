Pandark web crawler framework
=============================

> **Status (2026-10-02):** architecture skeleton accepted. **Not** a production crawler yet — treat as an **offline/fixture prototype** until the four acceptance gates in the design doc pass (Oak HTML, robots/sitemap fixtures, HTTP semantics fixtures, multi-host politeness fixtures).

Pandark collects and extracts web pages into `notedown-ir::DocumentGraph`. It is **not** Panduck: Panduck converts between document formats on local files, while Pandark crawls URLs and builds semantic documents from fetched content.

## Crates

- `pandark-types` — crawl contracts, `FetchArtifact`, frontier items, reports, and policies
- `pandark` — fetch transport, frontier orchestration, extract pipeline, and Node-API bindings

Browser sessions use a future `BrowserProvider` capability layer and are not part of the `pandark` crate surface yet.

## Developers

```bash
cargo test
cargo build --release
pnpm install
pnpm run build:napi
pnpm --filter @notedge/pandark test:e2e
```
