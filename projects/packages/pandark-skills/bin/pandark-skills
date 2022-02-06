#!/usr/bin/env node

/**
 * Install Pandark agent skills into local coding-agent skill directories.
 *
 * Usage:
 *   npx @notedge/pandark-skills
 *   npx @notedge/pandark-skills -g
 *   npx @notedge/pandark-skills -a cursor -y
 *
 * Extra args are forwarded to `npx skills add`.
 */

import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const userArgs = process.argv.slice(2);

if (userArgs.includes("--help") || userArgs.includes("-h")) {
    console.log(`@notedge/pandark-skills — install Pandark user agent skills

Usage:
  npx @notedge/pandark-skills
  npx @notedge/pandark-skills -g
  npx @notedge/pandark-skills -a cursor -y

This runs \`npx skills add <package-root> --skill pandark\` and forwards extra flags.
`);
    process.exit(0);
}

const hasSkillFlag = userArgs.some((arg) => arg === "--skill" || arg === "-s" || arg.startsWith("--skill="));

/** Quote one argv token for Windows cmd.exe when spawn(..., {shell:true}) joins args. */
function quoteForShell(value) {
    if (process.platform !== "win32") return value;
    if (!/[\s"&<>|^()]/.test(value)) return value;
    return `"${value.replace(/"/g, '""')}"`;
}

const skillsArgs = ["--yes", "skills", "add", quoteForShell(packageRoot)];
if (!hasSkillFlag) {
    skillsArgs.push("--skill", "pandark");
}
skillsArgs.push(...userArgs.map(quoteForShell));

const child = spawn("npx", skillsArgs, {
    stdio: "inherit",
    shell: true,
    env: process.env,
    windowsHide: true,
});

child.on("error", (error) => {
    console.error(`@notedge/pandark-skills: failed to launch skills CLI: ${error.message}`);
    process.exit(1);
});

child.on("exit", (code, signal) => {
    if (signal) {
        process.kill(process.pid, signal);
        return;
    }
    process.exit(code ?? 1);
});
