#!/usr/bin/env node
/**
 * Architecture boundary checks for the Pandark workspace.
 * gate: G-ARCH-1 / G-ARCH-2
 */

import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const CRATES_DIR = join(ROOT, "projects", "crates");

function fail(message) {
  console.error(`check-arch-boundaries: ${message}`);
  process.exitCode = 1;
}

function crateNames() {
  return readdirSync(CRATES_DIR, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name);
}

function readManifest(crate) {
  return readFileSync(join(CRATES_DIR, crate, "Cargo.toml"), "utf8");
}

function declaresDependency(manifest, name) {
  return new RegExp(`^${name}\\s*=`, "m").test(manifest);
}

function checkYydbOnlyInStore() {
  for (const crate of crateNames()) {
    const manifest = readManifest(crate);
    if (!declaresDependency(manifest, "yydb") && !manifest.includes("dep:yydb")) {
      continue;
    }
    if (crate !== "pandark-store") {
      fail(`crate \`${crate}\` must not declare a direct \`yydb\` dependency`);
    }
  }
  if (!declaresDependency(readManifest("pandark-store"), "yydb")) {
    fail("`pandark-store` must declare optional `yydb` dependency");
  }
}

function checkFetchDeps() {
  const manifest = readManifest("pandark-fetch");
  for (const forbidden of ["yydb", "notedown-ir", "pandark-store", "napi"]) {
    if (declaresDependency(manifest, forbidden)) {
      fail(`\`pandark-fetch\` must not depend on \`${forbidden}\``);
    }
  }
}

function listRustFiles(dir) {
  const out = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...listRustFiles(path));
      continue;
    }
    if (entry.isFile() && entry.name.endsWith(".rs")) {
      out.push(path);
    }
  }
  return out;
}

function checkNoYydbImportsOutsideStore() {
  for (const crate of crateNames()) {
    if (crate === "pandark-store") {
      continue;
    }
    const srcDir = join(CRATES_DIR, crate, "src");
    try {
      statSync(srcDir);
    } catch {
      continue;
    }
    for (const file of listRustFiles(srcDir)) {
      const text = readFileSync(file, "utf8");
      if (/\buse yydb\b/.test(text) || text.includes("yydb::Connection")) {
        fail(`forbidden yydb import in ${relative(ROOT, file)}`);
      }
    }
  }
}

checkYydbOnlyInStore();
checkFetchDeps();
checkNoYydbImportsOutsideStore();

if (process.exitCode) {
  process.exit(process.exitCode);
}

console.log("check-arch-boundaries: ok");
