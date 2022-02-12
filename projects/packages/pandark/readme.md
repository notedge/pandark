# @notedge/pandark

Agent-friendly TypeScript crawler for Node 20+. Install the package, call **`createPandark()`**, or run the **`pandark` CLI**—same engine, structured JSON output, crawl reports with budgets and checkpoints.

**Current release:** extract and inspect are the most reliable first steps. Treat site-wide crawling as prototype work until you verify scope, budget, and reports on your seeds.

## 🤖 Agent instructions

```bash
npx @notedge/pandark-skills
```

Installs agent instructions only—not this runtime.

## 📦 Install

```bash
npm install @notedge/pandark
npx pandark doctor
```

## 🧪 CLI

```bash
npx pandark extract ./page.html --output ./page.json --report ./page.report.json

npx pandark crawl https://example.com/docs \
  --depth 1 \
  --budget 50 \
  --report ./crawl.report.json
```

## 🔌 TypeScript

```ts
import { createPandark } from "@notedge/pandark";

const pandark = createPandark();

const extracted = pandark.extract("./page.html");
console.log(extracted.status, extracted.reportJson);

const crawled = pandark.crawl("https://example.com", { depth: 1, budget: 20 });
console.log(crawled.committedPages, crawled.reportJson);
```

`createPandark()` is the supported entry point. Lower-level exports (`loadPandarkNode`, `@notedge/pandark/cli`) are for integrators.

## 📊 Reports

Read `reportJson` for coverage, crawl counters, and pause reasons. `--report` on the CLI writes the same JSON to disk. A successful exit can still mean partial extraction—always inspect the report.

## 🔧 Troubleshooting

| Symptom | Check |
|---------|-------|
| Binding load fails | `npx pandark doctor` |
| Missing pages after crawl | Report lists `committedPages`; export documents yourself |
| Dynamic/login pages | Provide `--browser-endpoint` or `--browser-fixtures-dir` |

Repository: https://github.com/notedge/pandark

License: MPL-2.0
