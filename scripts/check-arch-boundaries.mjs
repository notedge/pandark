#!/usr/bin/env node
/**
 * Architecture boundary checks for the Pandark workspace.
 * gate: G-ARCH-1 / G-ARCH-2
 */

import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const CRATE_ROOTS = [
  join(ROOT, "projects", "pandark.rs", "crates"),
  join(ROOT, "projects", "pandark.rs", "exports"),
];

function fail(message) {
  console.error(`check-arch-boundaries: ${message}`);
  process.exitCode = 1;
}

function crateEntries() {
  const out = [];
  for (const root of CRATE_ROOTS) {
    for (const entry of readdirSync(root, { withFileTypes: true })) {
      if (entry.isDirectory()) {
        out.push({ name: entry.name, dir: join(root, entry.name) });
      }
    }
  }
  return out;
}

function readManifest(crateDir) {
  return readFileSync(join(crateDir, "Cargo.toml"), "utf8");
}

const DEPENDENCY_SECTIONS = new Set([
  "dependencies",
  "dev-dependencies",
  "build-dependencies",
]);

function isDependencySection(section) {
  if (DEPENDENCY_SECTIONS.has(section)) {
    return true;
  }
  return /^target\.(?:[^.[\]]+\.)*(?:dependencies|dev-dependencies|build-dependencies)$/.test(
    section,
  );
}

function declaredDependencies(manifest) {
  const names = new Set();
  let section = null;
  for (const rawLine of manifest.split(/\r?\n/)) {
    const header = rawLine.match(/^\s*\[(.+?)\]\s*$/);
    if (header) {
      section = header[1];
      continue;
    }
    if (section === null || !isDependencySection(section)) {
      continue;
    }
    const entry = rawLine.match(/^\s*([A-Za-z0-9_-]+)\s*=/);
    if (entry) {
      names.add(entry[1]);
    }
  }
  return names;
}

function declaresDependency(manifest, name) {
  return declaredDependencies(manifest).has(name);
}

function findCrate(name) {
  return crateEntries().find((entry) => entry.name === name);
}

function checkYydbOnlyInStore() {
  for (const crate of crateEntries()) {
    const manifest = readManifest(crate.dir);
    if (!declaresDependency(manifest, "yydb") && !manifest.includes("dep:yydb")) {
      continue;
    }
    if (crate.name !== "pandark-store") {
      fail(`crate \`${crate.name}\` must not declare a direct \`yydb\` dependency`);
    }
  }
  const store = findCrate("pandark-store");
  if (!store || !declaresDependency(readManifest(store.dir), "yydb")) {
    fail("`pandark-store` must declare optional `yydb` dependency");
  }
}

function checkFetchDeps() {
  const fetch = findCrate("pandark-fetch");
  if (!fetch) {
    fail("`pandark-fetch` crate is missing");
    return;
  }
  const manifest = readManifest(fetch.dir);
  for (const forbidden of ["yydb", "notedown-ir", "pandark-store", "napi"]) {
    if (declaresDependency(manifest, forbidden)) {
      fail(`\`pandark-fetch\` must not depend on \`${forbidden}\``);
    }
  }
}

function checkPandarkIsRlibOnly() {
  const facade = findCrate("pandark");
  if (!facade) {
    fail("`pandark` crate is missing");
    return;
  }
  const manifest = readManifest(facade.dir);
  if (manifest.includes("cdylib")) {
    fail("`pandark` must be an rlib facade and must not build a cdylib");
  }
  for (const forbidden of ["napi", "napi-derive"]) {
    if (declaresDependency(manifest, forbidden)) {
      fail(`\`pandark\` must not depend on \`${forbidden}\``);
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
  for (const crate of crateEntries()) {
    if (crate.name === "pandark-store") {
      continue;
    }
    const srcDir = join(crate.dir, "src");
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
checkPandarkIsRlibOnly();
checkNoYydbImportsOutsideStore();

if (process.exitCode) {
  process.exit(process.exitCode);
}

console.log("check-arch-boundaries: ok");
