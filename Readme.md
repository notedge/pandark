Pandark web crawler framework
=============================

Pandark collects and extracts web pages into `notedown-ir::DocumentGraph`. It is **not** Panduck: Panduck converts between document formats on local files, while Pandark crawls URLs and builds semantic documents from fetched content.

## Crates

- `pandark-types` — crawl seeds, budgets, fetch/extract outcomes, report types
- `pandark-core` — frontier planning and orchestration skeleton

## Developers

```bash
cargo test
cargo build --release
```
