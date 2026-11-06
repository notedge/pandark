import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
    PandarkWasmCapabilityError,
    instantiatePandarkWasm,
    loadPandarkWasm,
} from "../src/wasm/index.js";

const PACKAGE_ROOT = fileURLToPath(new URL("..", import.meta.url));
const platformKey = `${process.platform}-${process.arch}`;
const WASM_PATH = fileURLToPath(new URL(`../../../platforms/pandark-${platformKey}/lib/pandark_wasm.wasm`, import.meta.url));
const FACADE_VERSION = (JSON.parse(readFileSync(join(PACKAGE_ROOT, "package.json"), "utf8")) as { version: string }).version;
const SCHEMA_REPORT = "crawler.report/v0";

const PAGE_HTML = `<!doctype html>
<html lang="en">
  <head><title>Wasm gate</title></head>
  <body>
    <h1>Capability fixture</h1>
    <p>The WebAssembly adapter extracted this page with injected capabilities.</p>
  </body>
</html>
`;

const artifactBytes = (): Uint8Array<ArrayBuffer> => new Uint8Array(readFileSync(WASM_PATH));

test("G-ARCH-5 wasm artifact imports nothing and exports the ABI surface", () => {
    const module = new WebAssembly.Module(artifactBytes());
    assert.deepEqual(WebAssembly.Module.imports(module), [], "the wasm module must not import anything");
    const exports = WebAssembly.Module.exports(module).map((entry) => entry.name);
    for (const name of [
        "memory",
        "pandark_wasm_abi_version",
        "pandark_wasm_alloc",
        "pandark_wasm_dealloc",
        "pandark_wasm_run",
        "pandark_wasm_result_ptr",
        "pandark_wasm_result_len",
        "pandark_wasm_version_ptr",
        "pandark_wasm_version_len",
    ]) {
        assert.ok(exports.includes(name), `the wasm module must export ${name}`);
    }
});

test("G-ARCH-5 adapter version matches @notedge/pandark", async () => {
    const adapter = await instantiatePandarkWasm(artifactBytes());
    assert.equal(adapter.version, FACADE_VERSION, "the wasm version must match the npm package version");
    assert.equal(adapter.abiVersion, 1);
    assert.deepEqual(adapter.capabilities, ["fetch", "store", "clock", "cancel"]);
});

test("G-ARCH-5 capability fixture completes fetch -> extract -> report", async () => {
    const adapter = await instantiatePandarkWasm(artifactBytes());
    const seed = "https://example.com/";
    const outcome = adapter.run(
        { seeds: [seed], maxRequests: 4 },
        {
            pages: [
                {
                    url: seed,
                    finalUrl: seed,
                    status: 200,
                    contentType: "text/html",
                    headers: { "content-type": "text/html" },
                    body: PAGE_HTML,
                    provenance: "test",
                },
            ],
            clock: 1_700_000_000,
        },
    );

    const report = JSON.parse(outcome.reportJson) as Record<string, unknown>;
    assert.equal(report.schema_version, SCHEMA_REPORT, "the report envelope must keep the Crawler schema");
    assert.equal(report.extracted, 1, "the injected HTML page must extract");
    assert.equal(report.committed, 1, "the extracted document must reach the store capability");
    assert.equal(outcome.pages.length, 1);
    assert.equal(outcome.pages[0].finalUrl, seed);

    const pageKey = `pandark-wasm/page/${seed}`;
    const reportKey = `pandark-wasm/report/${seed}`;
    const stored = new Map(outcome.stored.map((record) => [record.key, record.payload]));
    assert.ok(stored.has(pageKey), "the document payload must be returned to the host");
    assert.ok(stored.has(reportKey), "the report envelope must be returned to the host");
    assert.ok(stored.get(pageKey)?.includes("Capability fixture"), "the document must keep the extracted heading text");
    assert.ok(outcome.reportJson.includes(SCHEMA_REPORT));
});

test("G-ARCH-5 missing fetch fixture raises a structured capability error", async () => {
    const adapter = await instantiatePandarkWasm(artifactBytes());
    assert.throws(
        () => adapter.run({ seeds: ["https://example.com/"], maxRequests: 1 }, { pages: [] }),
        (error: unknown) => {
            assert.ok(error instanceof PandarkWasmCapabilityError, "the failure must be a capability error");
            assert.equal(error.capability, "fetch");
            assert.ok(error.message.includes("no host page was injected"));
            return true;
        },
    );
});

test("G-ARCH-5 platform carrier resolution loads the packaged artifact", async () => {
    const adapter = await loadPandarkWasm();
    assert.equal(adapter.version, FACADE_VERSION);
});
