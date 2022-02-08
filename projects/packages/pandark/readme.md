# @notedge/pandark

Node 20+ package for Pandark: CLI commands and `loadPandarkNode()` bindings over the native crawler and extractor. Use
it when you need to extract saved HTML, plan a bounded crawl, or automate fetch and resume flows from JavaScript.

This release is an **offline/fixture prototype** for collection. Extraction and inspect paths are the most reliable
first tasks. Treat `--budget` as a per-run fetch cap, not a guarantee of politeness across hosts.

## 🤖 Agent instructions

Install `@notedge/pandark-skills` when you want a coding agent to choose commands, read reports, and explain partial
runs. That package installs instructions only—not this runtime.

```bash
npx @notedge/pandark-skills
```

## 📦 Install

```bash
npm install @notedge/pandark
npx pandark doctor
```

Platform-specific `.node` binaries come from optional dependencies (`@notedge/pandark-win32-x64`,
`@notedge/pandark-linux-x64`, `@notedge/pandark-darwin-arm64`, and siblings). npm installs the matching package when
your OS and CPU are supported.

## 🧪 Extract a saved page

Prerequisite: a readable HTML file and write permission for output paths.

```bash
npx pandark extract ./page.html \
  --output ./page.document.json \
  --report ./page.report.json
```

`--json` prints the combined result to stdout. Without `--output`, extract still writes the report (stdout or
`--report`).

## 🗺 Plan and crawl with bounds

Preview frontier expansion before spending requests:

```bash
npx pandark plan https://example.com/docs --depth 1 --budget 50 --json
```

Run a bounded crawl and capture diagnostics:

```bash
npx pandark crawl https://example.com/docs \
  --depth 1 \
  --budget 50 \
  --report ./crawl.report.json \
  --checkpoint-out ./crawl.checkpoint.json
```

Resume when a checkpoint exists:

```bash
npx pandark resume ./crawl.checkpoint.json --report ./crawl.resume.report.json
```

`--report` does not write every committed document to disk. Inspect `committedPages` in the binding response or report
JSON for what was committed.

## 🔌 Node API

```ts
import {loadPandarkNode} from "@notedge/pandark/node";

const pandark = loadPandarkNode();

const plan = pandark.planCrawl("https://example.com", 1, 20);
console.log(plan.frontier);

const extracted = pandark.extractInput("./page.html");
if (extracted.documentJson) {
    // notedown-ir document JSON
}

const crawled = pandark.crawlFile("https://example.com", 1, 20);
console.log(crawled.committedPages, crawled.reportJson);
```

Exports:

- `@notedge/pandark` — package metadata
- `@notedge/pandark/node` — `loadPandarkNode()`
- `@notedge/pandark/cli` — programmatic CLI builder

There is no WASM crawl export in this package. Browser crawl fallbacks require `--browser-endpoint` or
`--browser-fixtures-dir` on CLI/native paths.

## 📊 Reports and partial success

Read `reportJson` for extraction coverage, crawl counters, challenge handling, and pause reasons. Match CLI exit codes
with report status when automating—skipped or paused pages may not fail the process.

## 🔧 Troubleshooting

| Symptom                                            | Check                                                                                        |
|----------------------------------------------------|----------------------------------------------------------------------------------------------|
| `Unsupported platform for Pandark native bindings` | Install on a supported platform or add the matching `@notedge/pandark-*` optional dependency |
| `native bindings are not installed`                | Run `pandark doctor` and reinstall                                                           |
| Missing documents after crawl                      | Reports list committed URLs; export documents separately                                     |
| Dynamic page needs login                           | No built-in browser—provide an endpoint or fixture snapshots                                 |

Repository: https://github.com/notedge/pandark

License: MPL-2.0
