# pandark-browser-worker

Private render front-end for Pandark crawls. It serves the browser endpoint contract that
`HttpBrowserProvider` in `document.rs` already expects:

```
GET /?url=<page url>  ->  200 BrowserSnapshot JSON
```

Rendering uses the Playwright version pinned in `package.json` and its bundled Chromium in
headless mode. The worker never launches a host browser and never accepts `channel` or
`executablePath`. Extraction stays in `pandark` / `notedown-ir`: a snapshot is handed to
`extractInput(path, "snapshot")`, so the browser path does not introduce a second IR.

## 📤 Usage

```bash
# public targets only (loopback and private networks are denied)
node bin/pandark-browser-worker --port 9333

# local fixtures and local test servers
node bin/pandark-browser-worker --port 9333 --allow-private

# then point a bounded crawl at the worker
pandark crawl <seed> --browser-endpoint http://127.0.0.1:9333/ --browser-fallback on-fetch-failure
```

The snapshot JSON is `BrowserSnapshot` from `crawler.types`: `requested_url`, `final_url`,
`document_html`, `captured_at_epoch`, `browser_engine`, `profile_id`, `challenge_state`.

## 🧱 Render settle

A page counts as rendered when all of these hold, in this order:

1. `page.goto(url, { waitUntil: "load" })` returned without a navigation error.
2. `document.readyState === "complete"`.
3. An in-page `MutationObserver` records no DOM mutation for two consecutive animation frames.
   Settle budget is bounded (see below) and never uses a fixed sleep.

If the mutation observer hits its budget, the worker still returns `200` with
`challenge_state: "unstable_render"`, and `run_extract_from_snapshot` fails closed through the
configured challenge policy.

## 🛡️ Limits

| Limit | Value | Behaviour |
|-------|-------|-----------|
| `minPageTimeoutMs` | 1000 | lower input values are raised to this bound |
| `maxPageTimeoutMs` | 30000 | higher input values are clamped |
| `defaultPageTimeoutMs` | 20000 | default per-page navigation budget |
| `defaultSettleMs` | 5000 | default DOM settle budget |
| `maxSettleMs` | 10000 | settle budget upper bound |
| `quietFrames` | 2 | animation frames without mutation |
| `maxConcurrentCaptures` | 1 | one capture runs at a time in this worker |
| `maxQueuedCaptures` | 8 | further requests are rejected with `overloaded` |
| `maxSnapshotBytes` | 8 MiB | serialized `document_html` |
| `maxNetworkBytes` | 32 MiB | summed declared `content-length` for one page |
| `maxRedirects` | 4 | main-document redirect hops |
| `maxDomDepth` | 128 | deepest element nesting, well below the ~250 level crash point of this Chromium build |

## 🚨 Error codes

Failures return a JSON body and set `x-pandark-code`. Infrastructure failures use 4xx/5xx so
callers cannot mistake them for a rendered page.

| Code | Status | Meaning |
|------|--------|---------|
| `pandark.browser.request-invalid` | 400 | missing `url`, non-GET, non-http(s) scheme, unknown path |
| `pandark.browser.private-address-denied` | 403 | loopback / RFC1918 / link-local / unique-local target |
| `pandark.browser.overloaded` | 429 | capture queue is full |
| `pandark.browser.unavailable` | 503 | bundled Chromium failed to launch or the context failed |
| `pandark.browser.navigate-failed` | 502 | navigation or renderer failure, including a crashed target |
| `pandark.browser.render-timeout` | 504 | per-page navigation budget exhausted |
| `pandark.browser.redirect-limit` | 508 | more than `maxRedirects` main-document hops |
| `pandark.browser.dom-depth-limit` | 422 | DOM nesting exceeded `maxDomDepth` |
| `pandark.browser.snapshot-too-large` | 413 | serialized snapshot exceeded `maxSnapshotBytes` |
| `pandark.browser.page-budget` | 413 | declared response bytes exceeded `maxNetworkBytes` |

## 🔍 Evidence

`tests/browser-render.test.ts` runs local fixtures through both paths and writes
`evidence/browser-render-evidence.json`:

```bash
pnpm --filter @notedge/pandark-browser-worker test
```

The JSON records title, text characters, IR block nodes and body-marker hits for the direct
HTTP path and the rendered path, the guard responses above, and the real-site comparison that
is marked `blocked` when the network is unreachable.
