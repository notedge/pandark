# pandark-wasm

WASM adapter for Pandark. Exposes extract, inspect, and plan helpers for browser and Worker hosts.

Crawl orchestration is intentionally **not** exported from WASM because hosts cannot run a full crawler safely from a web page context.
