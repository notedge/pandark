# pandark-fetch

Transport layer for Pandark: HTTP, local file, and in-memory sources with optional response caching and robots policy
hooks.

Produces `pandark-types::FetchArtifact` values for `pandark-extract` and crawl orchestration in `pandark`.

## 🚚 Transports

| Transport | Typical input                           |
|-----------|-----------------------------------------|
| HTTP      | `https://` seeds and discovered links   |
| File      | Saved HTML or fixture snapshots on disk |
| Memory    | Tests and embedded fixture bytes        |

## 🤖 Robots and politeness

Robots.txt parsing and scheduling policies are implemented here, but **production politeness is not fully accepted** in
the current release. Treat robots handling as prototype behavior until fixture gates pass.

## 💾 Cache

Optional cache directories deduplicate repeated fetches during development and fixture runs. Cache semantics are
per-transport—do not assume a cache hit skips robots evaluation without verifying the release.

## 🔌 Integration

`pandark` selects transports from crawl options. `pandark-napi` exposes `fetchSeed` for single-seed diagnostics.

License: MPL-2.0
