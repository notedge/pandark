# pandark-napi

Node-API adapter for Pandark. Builds the platform `.node` binary published as `@notedge/pandark-<platform>` and loaded
by `@notedge/pandark/src/node/load.ts`.

## 📤 Exported operations

| Binding                        | Purpose                                      |
|--------------------------------|----------------------------------------------|
| `planCrawl`                    | Frontier preview with depth and budget       |
| `fetchSeed`                    | Single-seed fetch artifact JSON              |
| `crawlFile`                    | Bounded crawl with optional browser fixtures |
| `resumeCrawlFile`              | Continue from checkpoint JSON                |
| `extractInput` / `extractFile` | Semantic extraction to document JSON         |
| `inspectInput` / `inspectFile` | Stage reports without full extract           |

## 🔧 Build

From the repository root:

```bash
pnpm run build:napi
```

Copies `index.*.node` into `projects/packages/pandark-<platform>/lib/` for npm packaging.

## ⚠️ WASM boundary

Crawl bindings exist here only. `pandark-wasm` deliberately does not export crawl orchestration.

License: MPL-2.0
