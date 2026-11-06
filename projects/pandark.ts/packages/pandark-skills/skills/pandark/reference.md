# Pandark user reference

## npm packages

| Package                       | Role                                     |
|-------------------------------|------------------------------------------|
| `@notedge/pandark`            | TypeScript API and `createPandark()`     |
| `@notedge/pandark-<platform>` | Prebuilt native binary for your OS/CPU   |
| `@notedge/pandark-skills`     | Agent skill installer (this package)     |

## CLI commands

| Command           | Positional arg        | Purpose                             |
|-------------------|-----------------------|-------------------------------------|
| `pandark doctor`  | —                     | Node version + native binding check |
| `pandark plan`    | seed URL or seed file | Frontier preview, no I/O            |
| `pandark fetch`   | URL or path           | Single `FetchArtifact` JSON         |
| `pandark crawl`   | seed URL or seed file | Multi-page crawl + extract          |
| `pandark resume`  | checkpoint JSON path  | Continue paused crawl               |
| `pandark extract` | local file path       | HTML/snapshot → notedown-ir         |
| `pandark inspect` | local file path       | Probe, links, or extract-plan       |

Global-style flags used across commands:

- `--json` — machine-readable stdout wrapper
- `--report <file>` — write report JSON (`crawl`, `extract`, `resume`)

## `pandark crawl` flags

```
pandark crawl SEED
  [--depth <n>]
  [--budget <n>]
  [--browser-fallback never|on-fetch-failure]
  [--browser-fixtures-dir <dir>]
  [--browser-endpoint <url>]
  [--challenge-policy stop|pause-for-operator|skip-page|fallback-http]
  [--store <file.yydx>]
  [--report <file>]
  [--json]
```

## `pandark resume` flags

Same browser/challenge flags as crawl, but the positional arg is the run id printed by a store-backed crawl, and
`--store` is required:

```
pandark resume RUN_ID
  --store <file.yydx>
  [--browser-fallback …]
  [--challenge-policy …]
  [--browser-fixtures-dir <dir>]
  [--browser-endpoint <url>]
  [--report <file>]
  [--json]
```

The store keeps frontier, claims, committed pages and the original request. A resume re-claims only uncommitted
frontier records and rejects a mismatched `browser_fallback` strategy. There is no JSON checkpoint file.

## `pandark extract` / `inspect` flags

```
pandark extract INPUT
  [--from html|snapshot|auto]
  [--challenge-policy …]
  [-o, --output <file>]
  [--report <file>]
  [--json]

pandark inspect INPUT
  [--stage probe|links|extract-plan]
  [--from html|snapshot|auto]
  [--challenge-policy …]
  [--json]
```

## `PandarkClient` (supported API)

Create a client with `createPandark()` from `@notedge/pandark`:

```ts
import { createPandark } from "@notedge/pandark";

const pandark = createPandark();
```

```ts
type PandarkClient = {
    version: () => string;
    plan: (seed: string, depth?: number, budget?: number) => {
        frontier: string[];
        reportJson: string;
    };
    fetch: (seed: string) => {
        exitCode: number;
        artifactJson: string;
    };
    crawl: (seed: string, options?: {
        depth?: number;
        budget?: number;
        browserFixturesDir?: string;
        browserFallback?: string;
        challengePolicy?: string;
        browserEndpoint?: string;
        storePath?: string;
    }) => {
        exitCode: number;
        reportJson: string;
        committedPages: string[];
        runId?: string;
    };
    resume: (runId: string, options?: Omit<PandarkCrawlOptions, "depth" | "budget">) => {
        exitCode: number;
        reportJson: string;
        committedPages: string[];
        runId?: string;
    };
    extract: (inputPath: string, options?: {
        from?: string;
        sourceUrl?: string;
        challengePolicy?: string;
    }) => {
        exitCode: number;
        status: string;
        documentJson?: string;
        reportJson: string;
    };
    inspect: (inputPath: string, stage?: string, options?: {
        from?: string;
        challengePolicy?: string;
    }) => {
        exitCode: number;
        reportJson: string;
    };
};
```

Prefer **`createPandark()`** and these client methods. Lower-level native bindings are for integrators only.

## Browser snapshot fixture (minimal)

Used with `--browser-fixtures-dir` for offline or fallback capture. Filename convention in tests: slug derived from
URL + `.snapshot.json`.

```json
{
    "requested_url": "https://example.com/page",
    "final_url": "https://example.com/page",
    "document_html": "<html><head><title>…</title></head><body>…</body></html>",
    "captured_at_epoch": 1,
    "browser_engine": "fixture",
    "profile_id": "default",
    "challenge_state": "normal"
}
```

`challenge_state` examples: `normal`, `login_required`, `challenge_required`, `rate_limited`, `access_denied`.

## Crawl JSON stdout shape (with `--json`)

```json
{
    "exitCode": 0,
    "committedPages": ["https://example.com/"],
    "runId": "…",
    "report": { }
}
```

`runId` is present when `--store` was passed. On a pause (exit code 3) the run stays resumable in that store.

## Minimal probe script

```ts
import { createPandark } from "@notedge/pandark";

const pandark = createPandark();
console.log("version", pandark.version());
const { frontier } = pandark.plan("https://example.com", 1, 10);
console.log("frontier", frontier.length);
```

## Links

- Repository: https://github.com/notedge/pandark
- Product package: `@notedge/pandark` on npm (when published)
- Document conversion (different product): `@notedge/panduck-skills`
