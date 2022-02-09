# Pandark user reference

## npm packages

| Package                       | Role                                     |
|-------------------------------|------------------------------------------|
| `@notedge/pandark`            | TypeScript types and `loadPandarkNode()` |
| `@notedge/pandark/node`       | Node-API entry                           |
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
  [--checkpoint-out <file>]
  [--checkpoint-dir <dir>]
  [--cache-dir <dir>]
  [--report <file>]
  [--json]
```

## `pandark resume` flags

Same browser/challenge/cache/checkpoint flags as crawl, but positional arg is the checkpoint file:

```
pandark resume CHECKPOINT.json
  [--browser-fallback …]
  [--challenge-policy …]
  [--browser-fixtures-dir <dir>]
  [--browser-endpoint <url>]
  [--cache-dir <dir>]
  [--checkpoint-out <file>]
  [--checkpoint-dir <dir>]
  [--report <file>]
  [--json]
```

Auto-named checkpoints in `--checkpoint-dir` look like:

`pandark-checkpoint-<seed-slug>-<iso-timestamp>.json`

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

## `PandarkBindings` (current surface)

```ts
type PandarkBindings = {
    pandarkVersion: () => string;
    planCrawl: (seed: string, maxDepth?: number, maxRequests?: number) => {
        frontier: string[];
        reportJson: string;
    };
    fetchSeed: (seed: string) => {
        exitCode: number;
        artifactJson: string;
    };
    crawlFile: (
        seed: string,
        maxDepth?: number,
        maxRequests?: number,
        browserFixturesDir?: string,
        browserFallback?: string,
        challengePolicy?: string,
        browserEndpoint?: string,
        cacheDir?: string,
    ) => {
        exitCode: number;
        reportJson: string;
        committedPages: string[];
        checkpointJson?: string;
    };
    resumeCrawlFile: (
        checkpointJson: string,
        browserFixturesDir?: string,
        browserFallback?: string,
        challengePolicy?: string,
        browserEndpoint?: string,
        cacheDir?: string,
    ) => {
        exitCode: number;
        reportJson: string;
        committedPages: string[];
        checkpointJson?: string;
    };
    extractFile: (inputPath: string, sourceUrl?: string) => {
        exitCode: number;
        status: string;
        documentJson?: string;
        reportJson: string;
    };
    extractInput: (
        inputPath: string,
        from?: string,
        sourceUrl?: string,
        challengePolicy?: string,
    ) => {
        exitCode: number;
        status: string;
        documentJson?: string;
        reportJson: string;
    };
    inspectFile: (inputPath: string, stage?: string) => {
        exitCode: number;
        reportJson: string;
    };
    inspectInput: (
        inputPath: string,
        stage?: string,
        from?: string,
        challengePolicy?: string,
    ) => {
        exitCode: number;
        reportJson: string;
    };
};
```

Agents should read the installed package `src/types.ts` rather than assuming undocumented methods.

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
    "checkpoint": null,
    "checkpointPath": null,
    "report": { }
}
```

When paused, `checkpoint` is populated and `checkpointPath` is set if `--checkpoint-out` or `--checkpoint-dir` was used.

## Minimal probe script

```ts
import { loadPandarkNode } from "@notedge/pandark/node";

const p = loadPandarkNode();
console.log("version", p.pandarkVersion());
const { frontier } = p.planCrawl("https://example.com", 1, 10);
console.log("frontier", frontier.length);
```

## Links

- Repository: https://github.com/notedge/pandark
- Product package: `@notedge/pandark` on npm (when published)
- Document conversion (different product): `@notedge/panduck-skills`
