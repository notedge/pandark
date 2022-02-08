/**
 * Validate publishable Pandark npm package manifests and bin wiring.
 */

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const PUBLISHABLE = [
    "projects/packages/pandark",
    "projects/packages/pandark-skills",
    "projects/packages/pandark-win32-x64",
    "projects/packages/pandark-linux-x64",
    "projects/packages/pandark-linux-arm64",
    "projects/packages/pandark-darwin-x64",
    "projects/packages/pandark-darwin-arm64",
];

const MOJIBAKE = /鈥|�|ï¿½|\uFFFD/;
const BAD_BIN_SUFFIX = /\.mjs$/;

function fail(msg) {
    console.error(`check-npm-packages: ${msg}`);
    process.exitCode = 1;
}

function readJson(rel) {
    const abs = path.join(ROOT, rel);
    return { abs, dir: path.dirname(abs), raw: JSON.parse(fs.readFileSync(abs, "utf8")) };
}

let errors = 0;

for (const rel of PUBLISHABLE) {
    const { abs, dir, raw: pkg } = readJson(`${rel}/package.json`);
    const label = pkg.name ?? rel;

    if (typeof pkg.description !== "string" || !pkg.description.trim()) {
        fail(`${label}: missing description`);
        errors += 1;
    } else if (MOJIBAKE.test(pkg.description)) {
        fail(`${label}: description contains mojibake: ${JSON.stringify(pkg.description)}`);
        errors += 1;
    }

    if (pkg.private) {
        fail(`${label}: must not be private`);
        errors += 1;
    }

    if (pkg.publishConfig?.access !== "public") {
        fail(`${label}: publishConfig.access must be "public"`);
        errors += 1;
    }

    if (pkg.bin) {
        for (const [name, target] of Object.entries(pkg.bin)) {
            if (typeof target !== "string" || BAD_BIN_SUFFIX.test(target)) {
                fail(`${label}: bin[${name}] must be extensionless, got ${String(target)}`);
                errors += 1;
            }
            const binPath = path.join(dir, target);
            if (!fs.existsSync(binPath)) {
                fail(`${label}: missing bin file ${target}`);
                errors += 1;
            }
        }
    }

    if (Array.isArray(pkg.files)) {
        for (const entry of pkg.files) {
            if (entry === "lib" && /^@notedge\/pandark-/.test(label) && label !== "@notedge/pandark-skills") {
                continue;
            }
            const target = path.join(dir, entry);
            if (!fs.existsSync(target)) {
                fail(`${label}: files[] entry missing on disk: ${entry}`);
                errors += 1;
            }
        }
    }

    const binDir = path.join(dir, "bin");
    if (fs.existsSync(binDir)) {
        for (const name of fs.readdirSync(binDir)) {
            if (BAD_BIN_SUFFIX.test(name)) {
                fail(`${label}: remove duplicate bin/${name}; use extensionless bin entries only`);
                errors += 1;
            }
        }
    }

    if (rel.startsWith("projects/packages/pandark-") && rel !== "projects/packages/pandark-skills") {
        const main = pkg.main ?? "./index.js";
        const mainPath = path.join(dir, main);
        if (!fs.existsSync(mainPath)) {
            fail(`${label}: missing main ${main}`);
            errors += 1;
        }
    }

    console.log(`ok ${label} (${path.relative(ROOT, abs)})`);
}

if (errors > 0) {
    process.exit(process.exitCode ?? 1);
}

console.log(`check-npm-packages: ${PUBLISHABLE.length} packages validated`);
