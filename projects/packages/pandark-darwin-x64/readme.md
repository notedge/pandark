# @notedge/pandark-darwin-x64

Prebuilt Node-API binary for **macOS x64 (Intel)**. `@notedge/pandark` loads this package automatically on `darwin` +
`x64` through an optional dependency.

## 📦 Install

You normally do not install this package directly:

```bash
npm install @notedge/pandark
```

npm pulls `@notedge/pandark-darwin-x64` when the host matches `os: darwin` and `cpu: x64`.

## 🔧 Mismatch diagnosis

| Symptom                                            | Likely cause                                                 |
|----------------------------------------------------|--------------------------------------------------------------|
| `Unsupported platform for Pandark native bindings` | Wrong OS/CPU or optional dependency not installed            |
| Module loads but crawl fails                       | Runtime issue in `@notedge/pandark`—run `npx pandark doctor` |

Main package: https://www.npmjs.com/package/@notedge/pandark

License: MPL-2.0
