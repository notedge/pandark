import assert from "node:assert/strict";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawn } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";

const packageRoot = fileURLToPath(new URL("..", import.meta.url));
const cliEntry = join(packageRoot, "bin/pandark.mjs");

function runCli(args: string[]): Promise<{ code: number; stdout: string; stderr: string }> {
    return new Promise((resolve, reject) => {
        const child = spawn(process.execPath, [cliEntry, ...args], {
            cwd: packageRoot,
            env: process.env,
            stdio: ["ignore", "pipe", "pipe"],
        });
        let stdout = "";
        let stderr = "";
        child.stdout.on("data", (chunk) => {
            stdout += String(chunk);
        });
        child.stderr.on("data", (chunk) => {
            stderr += String(chunk);
        });
        child.on("error", reject);
        child.on("close", (code) => resolve({ code: code ?? 1, stdout, stderr }));
    });
}

test("doctor reports native binding when available", async () => {
    const result = await runCli(["doctor", "--json"]);
    assert.equal(result.code, 0);
    const payload = JSON.parse(result.stdout);
    assert.equal(payload.ok, true);
    assert.equal(payload.checks.some((check: { id: string }) => check.id === "native"), true);
});

test("extract writes document json from local html", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pandark-cli-"));
    const input = join(dir, "page.html");
    const output = join(dir, "page.json");
    await writeFile(
        input,
        "<html><head><title>T</title></head><body><p>hello</p></body></html>",
        "utf8",
    );

    const result = await runCli(["extract", input, "-o", output, "--json"]);
    assert.equal(result.code, 0);
    const payload = JSON.parse(result.stdout);
    assert.equal(payload.status, "complete");
    assert.ok(payload.document);
});
