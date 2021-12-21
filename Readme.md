Pandark web crawler framework
=============================

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
