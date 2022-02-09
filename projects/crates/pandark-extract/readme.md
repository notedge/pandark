# pandark-extract

Semantic extractors that turn `pandark-types::FetchArtifact` HTML (and related inputs) into `notedown-ir::DocumentGraph`
output plus extraction diagnostics.

Shared by the `pandark` facade, `pandark-napi`, and `pandark-wasm` so CLI, Node, and browser hosts see the same
extraction behavior.

## 🧪 Inputs and outputs

| Input                         | Output                                           |
|-------------------------------|--------------------------------------------------|
| Saved HTML / fetched artifact | `DocumentGraph` JSON + coverage/loss report      |
| Inspect stages                | Structural metadata without full semantic commit |

Heuristic HTML extraction may block or partial-commit on unknown markup. Always read the report—not only the exit code.

## ⚠️ Oak HTML maturity

Oak-backed HTML paths are under active acceptance testing. Fixture pages may extract cleanly while arbitrary production
sites still gap.

## 🔌 Integration

See the `pandark` facade and integration tests for how extractors are invoked in this release.

WASM exports extract and inspect only; crawl is intentionally excluded from `pandark-wasm`.

License: MPL-2.0
