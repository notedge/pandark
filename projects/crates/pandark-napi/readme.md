# pandark-napi

Node-API adapter for Pandark. Builds the platform `.node` binary published as `@notedge/pandark-<platform>` and loaded
by `@notedge/pandark/src/node/load.ts`.

## 📤 Exported operations

| Binding                        | Purpose                                      |
|--------------------------------|----------------------------------------------|
| `planCrawl`                    | Frontier preview with depth and budget       |
| `fetchSeed`                    | Single-seed fetch artifact JSON (Promise)    |
| `crawlFile`                    | Bounded crawl with optional browser fixtures (Promise) |
| `resumeCrawlFile`              | Continue from checkpoint JSON                |
| `extractInput` / `extractFile` | Semantic extraction to document JSON (Promise) |
| `inspectInput` / `inspectFile` | Stage reports without full extract           |

`fetchSeed`, `crawlFile`, `resumeCrawlFile`, `extractInput`, `extractFile` and
`extractToMarkdown` return Promises: the native work runs on the Node-API
thread pool, so the JavaScript event loop stays responsive while a request or
document parse is in flight.

## 🔧 Build

From the repository root:

```bash
pnpm run build:napi
```

Copies `index.*.node` into `projects/packages/pandark-<platform>/lib/` for npm packaging.
