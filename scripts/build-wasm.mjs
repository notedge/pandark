/**
 * Build `pandark-wasm` for `wasm32-unknown-unknown` and place the artifact in
 * every platform carrier's `lib/` directory.
 *
 * The module is dependency-free at the ABI level, so no bindgen tooling is
 * involved and the same bytes ship in all carriers.
 */

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const ARTIFACT = path.join(ROOT, "target", "wasm32-unknown-unknown", "release", "pandark_wasm.wasm");
const FACADE = path.join(ROOT, "projects", "pandark.ts", "packages", "pandark");
const PLATFORMS = path.join(ROOT, "projects", "pandark.ts", "platforms");

const REQUIRED_EXPORTS = [
    "memory",
    "pandark_wasm_abi_version",
    "pandark_wasm_alloc",
    "pandark_wasm_dealloc",
    "pandark_wasm_run",
    "pandark_wasm_result_ptr",
    "pandark_wasm_result_len",
    "pandark_wasm_version_ptr",
    "pandark_wasm_version_len",
];

function fail(message) {
    console.error(`build-wasm: ${message}`);
    process.exit(1);
}

function decode(memory, pointer, length) {
    return new TextDecoder().decode(new Uint8Array(memory.buffer, pointer, length).slice());
}

const build = spawnSync(
    "cargo",
    ["build", "-p", "pandark-wasm", "--target", "wasm32-unknown-unknown", "--release"],
    { cwd: ROOT, stdio: "inherit" },
);
if (build.status !== 0) {
    fail("cargo build failed. Install the target with `rustup target add wasm32-unknown-unknown --toolchain nightly`");
}
if (!fs.existsSync(ARTIFACT)) {
    fail(`missing artifact ${path.relative(ROOT, ARTIFACT)}`);
}

const bytes = fs.readFileSync(ARTIFACT);
const module = new WebAssembly.Module(bytes);
const imports = WebAssembly.Module.imports(module);
if (imports.length > 0) {
    fail(`the wasm module must not import anything, got: ${JSON.stringify(imports)}`);
}
const exported = new Set(WebAssembly.Module.exports(module).map((entry) => entry.name));
for (const name of REQUIRED_EXPORTS) {
    if (!exported.has(name)) {
        fail(`the wasm module is missing export \`${name}\``);
    }
}

const instance = await WebAssembly.instantiate(module, {});
const facadeVersion = JSON.parse(fs.readFileSync(path.join(FACADE, "package.json"), "utf8")).version;
const abiVersion = instance.exports.pandark_wasm_abi_version();
const version = decode(instance.exports.memory, instance.exports.pandark_wasm_version_ptr(), instance.exports.pandark_wasm_version_len());
if (abiVersion !== 1) {
    fail(`unexpected ABI version ${abiVersion}`);
}
if (version !== facadeVersion) {
    fail(`wasm version ${version} does not match @notedge/pandark ${facadeVersion}`);
}

const carriers = fs
    .readdirSync(PLATFORMS, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && entry.name.startsWith("pandark-"))
    .map((entry) => entry.name);
for (const carrier of carriers) {
    const lib = path.join(PLATFORMS, carrier, "lib");
    fs.mkdirSync(lib, { recursive: true });
    fs.copyFileSync(ARTIFACT, path.join(lib, "pandark_wasm.wasm"));
}

console.log(`build-wasm: pandark_wasm.wasm ${version} (ABI ${abiVersion}) copied to ${carriers.length} carriers`);
