# pandark

Rust crawl orchestration facade for Pandark. Wires `pandark-fetch` transports, `pandark-extract` semantic extractors,
and optional `pandark-store` persistence into plan, fetch, crawl, resume, extract, and inspect flows consumed by
`pandark-napi` and tests.

CLI users should install `@notedge/pandark` instead of depending on this crate directly.

## 🧩 Ownership

| Concern                              | Crate             |
|--------------------------------------|-------------------|
| Contracts (`FetchArtifact`, reports) | `pandark-types`   |
| HTTP/file/memory fetch               | `pandark-fetch`   |
| HTML → `notedown-ir`                 | `pandark-extract` |
| Checkpoints and crawl storage        | `pandark-store`   |
| Node-API surface                     | `pandark-napi`    |
| Browser extract/plan only            | `pandark-wasm`    |

Document semantics live in `notedown-ir::DocumentGraph`, not in this crate.

## 🔌 Integration

Downstream adapters call the public Rust API to run bounded crawls and extraction. Browser sessions go through a
`BrowserProvider` capability on native paths only.

See `pandark` crate docs and `projects/crates/pandark/tests/` for the current public Rust entry points.

## ⚠️ Release status

Collection behavior is still validated against fixtures and small scopes. Do not treat politeness, Oak HTML extraction,
or multi-host scheduling as production-complete from this facade alone.

Repository: https://github.com/notedge/pandark

License: MPL-2.0
