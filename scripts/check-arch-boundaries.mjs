/**
 * Architecture gate for the pinned Crawler component graph.
 *
 * The shared base must stay free of persistence backends, Node ABI and
 * WebAssembly glue, and component dependencies must stay one-way. The check
 * reads `cargo metadata` so it covers direct and transitive edges of the
 * pinned `document.rs` revision.
 */

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const FORBIDDEN_IN_BASE = ["yydb", "napi", "napi-derive", "wasm-bindgen", "crawler", "crawler-fetch", "crawler-extract", "crawler-store", "crawler-frontier", "crawler-browser"];

/** Allowed Crawler-crate edges, from importer to imported. */
const ALLOWED_COMPONENT_EDGES = {
    "crawler-types": [],
    "crawler-fetch": ["crawler-types"],
    "crawler-extract": ["crawler-types"],
    "crawler-store": ["crawler-types"],
    "crawler-frontier": ["crawler-types"],
    "crawler-browser": ["crawler-types", "crawler-extract"],
};

let errorCount = 0;
const fail = (message) => {
    console.error(`check-arch-boundaries: ${message}`);
    errorCount += 1;
};

const metadata = spawnSync("cargo", ["metadata", "--format-version", "1"], { cwd: ROOT, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
if (metadata.status !== 0) {
    console.error(metadata.stderr);
    fail("cargo metadata failed");
    process.exit(1);
}

const parsed = JSON.parse(metadata.stdout);
const packages = new Map(parsed.packages.map((entry) => [entry.name, entry]));
const nodes = new Map((parsed.resolve?.nodes ?? []).map((node) => [node.id, node]));
const ids = new Map(parsed.packages.map((entry) => [entry.name, entry.id]));

if (!packages.has("crawler-types")) {
    fail("the pinned graph is missing `crawler-types`");
    process.exit(1);
}

const directDependencies = (name) => (packages.get(name)?.dependencies ?? []).map((entry) => entry.name);

const closure = (name) => {
    const id = ids.get(name);
    const seen = new Set();
    const queue = [...(nodes.get(id)?.deps ?? [])].map((dep) => dep.pkg);
    while (queue.length > 0) {
        const next = queue.pop();
        if (seen.has(next)) continue;
        seen.add(next);
        for (const dep of nodes.get(next)?.deps ?? []) {
            queue.push(dep.pkg);
        }
    }
    return [...seen].map((entry) => parsed.packages.find((pkg) => pkg.id === entry)?.name).filter(Boolean);
};

for (const name of FORBIDDEN_IN_BASE) {
    if (closure("crawler-types").includes(name)) {
        fail(`\`crawler-types\` must not reach \`${name}\` through its dependency closure`);
    }
}

for (const [name, allowed] of Object.entries(ALLOWED_COMPONENT_EDGES)) {
    const packageEntry = packages.get(name);
    if (!packageEntry) continue;
    const crawlerDeps = directDependencies(name).filter((dependency) => dependency.startsWith("crawler-"));
    for (const dependency of crawlerDeps) {
        if (!allowed.includes(dependency)) {
            fail(`\`${name}\` must not depend on \`${dependency}\`. allowed: ${allowed.length > 0 ? allowed.join(", ") : "none"}`);
        }
    }
}

const manifest = fs.readFileSync(path.join(ROOT, "Cargo.toml"), "utf8");
const pin = manifest.match(/crawler-types\s*=\s*\{\s*git\s*=\s*"[^"]+",\s*rev\s*=\s*"([^"]+)"/)?.[1];
if (!pin) {
    fail("`crawler-types` must stay pinned to an explicit revision in `Cargo.toml`");
}

if (errorCount > 0) {
    process.exit(1);
}
console.log(`check-arch-boundaries: crawler-types closure is clean at pin ${pin}`);
