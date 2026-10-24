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

### Crawler (multi-page)

`Crawler` is a thin authoring surface over the Rust crawl runtime. Rust owns frontier scheduling, scope gates, politeness, and budgets. Setting `scope`, `limits`, or a crawler-level `politeness` routes the run through `crawlFile`, and every committed page is delivered to your `handlePage` callback with the same `Page` shape as single-page crawls.

```ts
import { Crawler } from "@notedge/pandark";

const crawler = new Crawler({ politeness: "balanced" });
const run = await crawler.crawl("https://example.com/docs", {
  scope: { allowedHosts: ["example.com"] },
  limits: { maxDepth: 2, maxRequests: 50 },
  handlePage: async (page) => ({ text: await page.html() }),
});

console.log(run.status, run.budgetStatus, run.remainingFrontier);
```

`CrawlRun.budgetStatus` is `completed`, `exhausted`, or `cancelled`. A budget-exhausted run reports `success_unvalidated` and carries a `pandark.crawl.budget-exhausted` diagnostic with `remainingFrontier`—it never claims full success.

## 📊 Reports

Read `reportJson` for coverage, crawl counters, and pause reasons. `--report` on the CLI writes the same JSON to disk. A successful exit can still mean partial extraction—always inspect the report.

## 🔧 Troubleshooting

| Symptom | Check |
|---------|-------|
| Binding load fails | `npx pandark doctor` |
| Missing pages after crawl | Report lists `committedPages`; export documents yourself |
| Dynamic/login pages | Provide `--browser-endpoint` or `--browser-fixtures-dir` |

Repository: https://github.com/notedge/pandark
