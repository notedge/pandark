import assert from "node:assert/strict";
import { readFileSync, readdirSync, existsSync } from "node:fs";
import { join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const PACKAGE_ROOT = fileURLToPath(new URL("..", import.meta.url));
const REPO_ROOT = resolve(PACKAGE_ROOT, "../../../..");
const PLATFORMS = join(REPO_ROOT, "projects", "pandark.ts", "platforms");

const readJson = (path: string): Record<string, unknown> => JSON.parse(readFileSync(path, "utf8"));

const PLATFORM_CARRIERS: Record<string, { os: string; cpu: string }> = {
    "pandark-win32-x64": { os: "win32", cpu: "x64" },
    "pandark-linux-x64": { os: "linux", cpu: "x64" },
    "pandark-linux-arm64": { os: "linux", cpu: "arm64" },
    "pandark-darwin-x64": { os: "darwin", cpu: "x64" },
    "pandark-darwin-arm64": { os: "darwin", cpu: "arm64" },
};

test("G-ARCH-5 facade and every platform carrier share one version", () => {
    const facade = readJson(join(PACKAGE_ROOT, "package.json"));
    const version = facade.version as string;
    assert.match(version, /^\d+\.\d+\.\d+$/, "the facade version must be a release version");

    for (const carrier of readdirSync(PLATFORMS, { withFileTypes: true }).filter((entry) => entry.isDirectory())) {
        if (!carrier.name.startsWith("pandark-")) continue;
        const manifest = readJson(join(PLATFORMS, carrier.name, "package.json"));
        assert.equal(manifest.version, version, `${carrier.name} must share the facade version`);
        assert.ok(existsSync(join(PLATFORMS, carrier.name, manifest.main as string)), `${carrier.name} main must exist`);
        assert.deepEqual(manifest.files, ["index.js", "lib", "readme.md"], `${carrier.name} must publish its lib directory`);
    }

    const workspaceManifest = readFileSync(join(REPO_ROOT, "Cargo.toml"), "utf8");
    const workspaceVersion = workspaceManifest.match(/\[workspace\.package\][\s\S]*?version\s*=\s*"([^"]+)"/)?.[1];
    assert.equal(workspaceVersion, version, "the Rust workspace version must match the npm packages");

    const wasmManifest = readFileSync(join(REPO_ROOT, "projects", "pandark.rs", "pandark-wasm", "Cargo.toml"), "utf8");
    assert.match(wasmManifest, /^version\.workspace = true$/m, "`pandark-wasm` must inherit the workspace version");
});

test("G-ARCH-5 facade optional dependencies cover every platform carrier", () => {
    const facade = readJson(join(PACKAGE_ROOT, "package.json"));
    const optional = facade.optionalDependencies as Record<string, string>;
    assert.ok(optional, "the facade must declare platform carriers as optional dependencies");
    assert.deepEqual(Object.keys(optional).sort(), Object.keys(PLATFORM_CARRIERS).map((name) => `@notedge/${name}`).sort());
});

test("G-ARCH-5 platform carriers declare the matching os and cpu", () => {
    for (const [name, expected] of Object.entries(PLATFORM_CARRIERS)) {
        const manifest = readJson(join(PLATFORMS, name, "package.json"));
        assert.deepEqual(manifest.os, [expected.os], `${name} must declare os ${expected.os}`);
        assert.deepEqual(manifest.cpu, [expected.cpu], `${name} must declare cpu ${expected.cpu}`);
    }
});

test("G-ARCH-5 facade exports the Node and WebAssembly entries", () => {
    const facade = readJson(join(PACKAGE_ROOT, "package.json"));
    const exports = facade.exports as Record<string, string>;
    assert.equal(exports["./node"], "./src/node/index.ts");
    assert.equal(exports["./wasm"], "./src/wasm/index.ts");
    assert.equal(exports["./cli"], "./src/cli/index.ts");
});
