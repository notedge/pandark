Pandark web crawler framework
=============================

Pandark collects and extracts web pages into `notedown-ir::DocumentGraph`. It is **not** Panduck: Panduck converts between document formats on local files, while Pandark crawls URLs and builds semantic documents from fetched content.

## Crates

- `pandark-types` — seeds, `FetchArtifact`, frontier, extract contracts, crawl reports
- `pandark-fetch` — HTTP/file transport, robots, response cache
- `pandark-core` — frontier admission, crawl orchestration, offline extract
- `pandark-napi` — Node-API bindings
- `@notedge/pandark` — TypeScript CLI and native loader

Browser sessions and controlled collection are specified separately via `BrowserProvider` and are not part of `pandark-core`.

## Developers

```bash
cargo test
cargo build --release
pnpm install
pnpm run build:napi
pnpm --filter @notedge/pandark test:e2e
```
