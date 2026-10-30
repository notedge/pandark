---
name: pandark
description: Help users crawl and extract web pages with Pandark (@notedge/pandark). Install the CLI or Node bindings, plan and run crawls, fetch single URLs, extract notedown-ir from HTML or browser snapshots, pause and resume with checkpoints, and explain crawl reports in plain language. Load when the user mentions Pandark, web crawling, scraping, seed URLs, browser fallback, notedown-ir extraction, or checkpoint resume — not Panduck document conversion.
---

# Pandark for users

## One sentence

**Pandark turns URLs and browsable pages into notedown-ir document graphs and crawl reports — it crawls and extracts, it
does not convert DOCX/EPUB/Markdown like Panduck.**

## Pandark vs Panduck

|        | Pandark                                  | Panduck                             |
|--------|------------------------------------------|-------------------------------------|
| Input  | URL, seed file, saved HTML/snapshot      | Local file in a known format        |
| Job    | Fetch, discover links, extract semantics | Decode and convert between formats  |
| Output | notedown-ir + crawl report               | Target format file + convert report |

If the user only needs "Markdown → HTML" or "Word → PDF", point them to Panduck (`@notedge/panduck-skills`), not
Pandark.

## When to use this skill

The user has a **collection job**, not a Rust monorepo task. Typical goals:

- Crawl a site or a list of seed URLs with depth and request budget
- Fetch one page for debugging redirects or headers
- Extract semantic IR from a saved HTML file or browser snapshot JSON
- Inspect links or extractors before a full crawl
- Pause on login/challenge pages and resume after operator action
- Wire Pandark into a Node script or CI step
- Fix install errors for `@notedge/pandark` platform packages

Do **not** default to internal crate names, frontier internals, or contributor workflows unless the user explicitly asks
to hack on the Pandark repository.

## Install

**npm package:** `@notedge/pandark`

```bash
npm install @notedge/pandark
# or
pnpm add @notedge/pandark
```

The CLI binary is `pandark`. Native speed uses an optional platform package (`@notedge/pandark-win32-x64`,
`@notedge/pandark-darwin-arm64`, …) pulled in automatically when supported.

**This skill package** (teaches agents how to help Pandark users):

```bash
npx @notedge/pandark-skills
npx @notedge/pandark-skills -a cursor -y
```

**First step after install:** run `pandark doctor` (or `pandark doctor --json`). If native bindings fail, do not promise
crawl speed or full functionality until the platform package is available.

Build native artifacts from source only when the user is developing Pandark itself: `pnpm run build:napi` at the repo
root.

## Quick start (CLI)

```bash
# Sanity check
pandark doctor

# Plan without network I/O
pandark plan https://example.com --depth 2 --json

# Fetch one URL (artifact JSON)
pandark fetch https://example.com/page.html --json

# Crawl with machine-readable output
pandark crawl https://example.com --depth 1 --budget 50 --report crawl.report.json --json

# Extract local HTML to notedown-ir JSON on stdout path
pandark extract page.html --from html -o page.nd.json --json

# Inspect links without writing IR
pandark inspect page.html --stage links --json
```

## Quick start (Node)

```ts
import { createPandark } from "@notedge/pandark";

const pandark = createPandark();
console.log(pandark.version());

const plan = pandark.plan("https://example.com", 2, 100);
console.log(plan.frontier);

const crawl = pandark.crawl("https://example.com", {
    depth: 1,
    budget: 20,
    browserFallback: "on-fetch-failure",
    challengePolicy: "stop",
});
console.log(crawl.exitCode, crawl.committedPages);
```

Use **`createPandark()`** and its client methods. See [reference.md](reference.md) for the full method list.

## Seed input

`crawl` and `plan` accept one positional argument:

| Form                       | Meaning                     |
|----------------------------|-----------------------------|
| `https://…` or `http://…`  | Single seed URL             |
| `file://…`                 | Single local file URL       |
| Path to a **regular file** | Newline-delimited seed list |

Seed file rules:

- One URL or path per line
- Blank lines ignored
- Lines starting with `#` are comments
- Non-URL lines in a seed file are normalized to `file://` paths

Example `seeds.txt`:

```text
# docs batch
https://example.com/a
https://example.com/b
```

## How to help the user

### 1. Clarify the job

Ask only what affects the crawl or extract:

- Seed URL (s) or seed file path
- Depth and request budget (how wide and how many HTTP calls)
- Must-use browser for JS/login, or plain HTTP is enough
- What to do on login, CAPTCHA, or rate limits
- One-off page vs multi-page crawl vs resume from checkpoint
- Runtime: shell CLI, Node script, or CI

### 2. Pick the command

| Goal                             | Command                          |
|----------------------------------|----------------------------------|
| Preview URL frontier, no network | `pandark plan SEED`              |
| Debug one fetch                  | `pandark fetch URL`              |
| Multi-page collection            | `pandark crawl SEED`             |
| Continue after operator pause    | `pandark resume CHECKPOINT.json` |
| Local HTML → IR                  | `pandark extract FILE`           |
| Probe extractors / links         | `pandark inspect FILE`           |
| Install / platform check         | `pandark doctor`                 |

Always prefer `pandark … --json` when the user or agent needs structured output.

### 3. Crawl options that matter

| Flag                           | Values                      | When to suggest                          |
|--------------------------------|-----------------------------|------------------------------------------|
| `--depth <n>`                  | integer                     | Limit link hops from seeds               |
| `--budget <n>`                 | integer                     | Cap total HTTP requests                  |
| `--browser-fallback`           | `never`, `on-fetch-failure` | Use browser when HTTP fetch fails        |
| `--browser-fixtures-dir <dir>` | path                        | Offline/dev: JSON snapshots keyed by URL |
| `--browser-endpoint <url>`     | HTTP base URL               | Remote browser worker (when deployed)    |
| `--challenge-policy`           | see below                   | Behavior on login/challenge pages        |
| `--checkpoint-out <file>`      | path                        | Exact checkpoint file on pause           |
| `--checkpoint-dir <dir>`       | path                        | Auto-named checkpoint on pause           |
| `--cache-dir <dir>`            | path                        | Persistent fetch cache for crawl/resume  |
| `--report <file>`              | path                        | Write crawl JSON report to disk          |

**Challenge policies** (CLI uses kebab-case):

- `stop` — stop the run and report
- `pause-for-operator` — pause and emit checkpoint (exit code 3)
- `skip-page` — skip the page and continue
- `fallback-http` — try plain HTTP when browser sees a challenge

### 4. Pause and resume workflow

When `--challenge-policy pause-for-operator` hits a challenge page:

1. Crawl exits with code **3**
2. With `--checkpoint-out` or `--checkpoint-dir`, a checkpoint JSON is written
3. User resolves login/CAPTCHA out of band (or supplies new browser snapshots)
4. Resume with the **same** `--browser-fallback` and related flags:

```bash
pandark resume ./checkpoints/pandark-checkpoint-example-com-….json \
  --browser-fallback on-fetch-failure \
  --challenge-policy pause-for-operator \
  --checkpoint-dir ./checkpoints \
  --json
```

Resume rejects a mismatched `browser_fallback` strategy vs the checkpoint fingerprint. Keep flags consistent across
crawl and resume.

### 5. Extract and inspect

```bash
# Input format: html, snapshot, or auto
pandark extract saved.html --from auto -o doc.json --report extract.report.json

# Inspect stages: probe (default), links, extract-plan
pandark inspect saved.html --stage extract-plan --from html --json
```

`--from snapshot` expects browser snapshot JSON (same shape as fixture files under `--browser-fixtures-dir`).

### 6. Report results honestly

Users care about **outcomes**, not internal IR type names.

Summarize when relevant:

- **committedPages** — URLs successfully extracted
- **failed / skipped** counts from the report JSON
- **checkpoint** — run paused for operator; not a hard failure
- **Partial HTML extraction** — current HTML path is heuristic; complex pages may lose structure until Oak-based
  extraction ships

If `--json` is used, parse `report` and `exitCode` from stdout instead of guessing.

## Exit codes (current CLI)

| Code | Meaning                                                       |
|------|---------------------------------------------------------------|
| 0    | Success                                                       |
| 1    | Invalid arguments or bad seed/checkpoint input                |
| 2    | Crawl/extract failed (e.g. failed pages in report)            |
| 3    | Paused awaiting resume **or** run finished with skipped pages |
| 4    | Internal error (often missing native bindings)                |

Treat exit **3** carefully: read `checkpoint` / `checkpointPath` in JSON output to see if resume is needed.

## Example user prompts

```text
Crawl https://docs.example.com with depth 2 and budget 100. Write the report to crawl.json and print JSON summary.
```

```text
I have seeds.txt with 20 URLs. Batch crawl with on-fetch-failure and fixtures in ./snapshots.
```

```text
Pandark paused with exit code 3. Where is the checkpoint and what resume command do I run?
```

```text
Extract this downloaded HTML to notedown-ir and tell me if any links were discovered.
```

```text
Does Pandark replace Panduck for converting my EPUB library? (Answer: no — explain the split.)
```

## Troubleshooting

| Symptom                             | What to check                                                                 |
|-------------------------------------|-------------------------------------------------------------------------------|
| `native bindings are not installed` | Run `pandark doctor`; OS/arch may lack `@notedge/pandark-<platform>`          |
| Crawl commits 0 pages               | Network blocked, robots/deny rules, or HTTP failures without browser fallback |
| Exit 3 with checkpoint              | Expected for `pause-for-operator`; use `resume` with matching flags           |
| `resume strategy mismatch`          | Use the same `--browser-fallback` as the original crawl                       |
| Empty or thin extract               | HTML extractor is still skeleton-grade; inspect with `--stage links` first    |
| Seed file "no usable seeds"         | File only comments/blanks, or wrong path                                      |

## Acceptance status (2026-10-02)

**Architecture skeleton: pass. Production crawler: fail.**

Pandark is an **offline/fixture prototype** today. Agents must not describe it as a production-ready crawler framework.

| Area           | Today                                | Not production-ready because                                                   |
|----------------|--------------------------------------|--------------------------------------------------------------------------------|
| HTML extract   | String heuristics in `HtmlExtractor` | No Oak HTML view or `oak-css-selector`                                         |
| Robots         | Prefetch + parse skeleton            | `crawl-delay` not scheduled, sitemap not consumed, no durable robots cache/TTL |
| Browser        | Snapshot fixture / HTTP worker stub  | No login lifecycle, session, or real headless runtime                          |
| Frontier       | Local queue + depth/budget           | No per-host token bucket, `Retry-After`, backoff, or fair scheduling           |
| Cache / resume | Memory + optional `--cache-dir` disk | No conditional revalidation, full checkpoint page/asset state                  |

**Four gates before "production crawler" language:**

1. Oak HTML + selector fixtures
2. Real robots/sitemap fixture suite
3. HTTP retry, compression, charset, conditional requests, SSRF fixtures
4. Multi-host politeness, backoff, cancel, and resume fixtures

Until those pass, recommend Pandark for **local HTML extract**, **fixture-driven crawl demos**, and **pipeline
prototyping** — not unattended production site harvesting.

## Honest limits (current release)

Do **not** promise these until the acceptance gates pass:

- Full Playwright/CDP login flows out of the box (browser worker is minimal)
- `pandark crawl -o OUTPUT_DIR` bulk write of IR files to a tree (use `--report` + post-process or Node API)
- Production-grade robots compliance, sitemap discovery, per-host rate limiting, distributed crawl
- WASM/browser build of Pandark (Node native only today)

Say clearly when a workaround is "save HTML yourself, then `pandark extract`".

## Agent discipline

- **User-first language** — "your seed URL", "the crawl report", not `FrontierQueue` or `CrawlCheckpoint`.
- **Check doctor first** — before long crawls on a new machine.
- **No fake APIs** — only methods on `PandarkBindings` and documented CLI flags.
- **Respect sites** — suggest conservative `--budget` and `--depth`; remind about terms of service and robots when
  relevant.
- **Preserve user data** — do not overwrite checkpoints or reports without confirmation.
- **Pandark ≠ Panduck** — never suggest Pandark for format conversion jobs.

## More detail

See [reference.md](reference.md) for CLI flag cheat sheet, `PandarkBindings` fields, snapshot fixture shape, and policy
strings.
