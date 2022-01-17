import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
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

test("inspect reports links from local html", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pandark-cli-"));
    const input = join(dir, "page.html");
    await writeFile(
        input,
        '<html><body><a href="/docs">docs</a></body></html>',
        "utf8",
    );

    const result = await runCli(["inspect", input, "--stage", "links", "--json"]);
    assert.equal(result.code, 0);
    const payload = JSON.parse(result.stdout);
    assert.equal(payload.report.stage, "links");
    assert.ok(payload.report.discovered_links.length >= 1);
});

test("extract blocks login_required snapshot with stop policy", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pandark-cli-"));
    const input = join(dir, "login.snapshot.json");
    await writeFile(
        input,
        JSON.stringify({
            requested_url: "https://example.com/private",
            final_url: "https://example.com/login",
            document_html: "<html><body>login</body></html>",
            captured_at_epoch: 1,
            browser_engine: "fixture",
            profile_id: "test",
            challenge_state: "login_required",
        }),
        "utf8",
    );

    const result = await runCli([
        "extract",
        input,
        "--from",
        "snapshot",
        "--challenge-policy",
        "stop",
        "--json",
    ]);
    assert.equal(result.code, 2);
    const payload = JSON.parse(result.stdout);
    assert.equal(payload.status, "challenge_blocked");
    assert.equal(payload.report.challenge_state, "login_required");
    assert.equal(payload.report.challenge_outcome, "stop");
});

test("inspect reports challenge outcome for login snapshot", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pandark-cli-"));
    const input = join(dir, "login.snapshot.json");
    await writeFile(
        input,
        JSON.stringify({
            requested_url: "https://example.com/private",
            final_url: "https://example.com/login",
            document_html: "<html><body>login</body></html>",
            captured_at_epoch: 1,
            browser_engine: "fixture",
            profile_id: "test",
            challenge_state: "login_required",
        }),
        "utf8",
    );

    const result = await runCli([
        "inspect",
        input,
        "--from",
        "snapshot",
        "--challenge-policy",
        "stop",
        "--json",
    ]);
    assert.equal(result.code, 2);
    const payload = JSON.parse(result.stdout);
    assert.equal(payload.report.challenge_state, "login_required");
    assert.equal(payload.report.challenge_outcome, "stop");
});

test("extract reads browser snapshot json", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pandark-cli-"));
    const input = join(dir, "page.snapshot.json");
    const output = join(dir, "page.json");
    await writeFile(
        input,
        JSON.stringify({
            requested_url: "https://example.com/",
            final_url: "https://example.com/",
            document_html:
                "<html><head><title>Snap</title></head><body><p>snap</p></body></html>",
            captured_at_epoch: 1,
            browser_engine: "fixture",
            profile_id: "test",
            challenge_state: "normal",
        }),
        "utf8",
    );

    const result = await runCli(["extract", input, "--from", "snapshot", "-o", output, "--json"]);
    assert.equal(result.code, 0);
    const payload = JSON.parse(result.stdout);
    assert.equal(payload.status, "complete");
    assert.ok(payload.document);
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

test("crawl uses browser fixtures when http fetch fails", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pandark-cli-"));
    const fixturesDir = join(dir, "fixtures");
    await mkdir(fixturesDir);
    const seed = "http://127.0.0.1:1/pandark-fixture-page";
    await writeFile(
        join(fixturesDir, "page.snapshot.json"),
        JSON.stringify({
            requested_url: seed,
            final_url: seed,
            document_html:
                "<html><head><title>Fixture</title></head><body><p>offline</p></body></html>",
            captured_at_epoch: 1,
            browser_engine: "fixture",
            profile_id: "test",
            challenge_state: "normal",
        }),
        "utf8",
    );

    const result = await runCli([
        "crawl",
        seed,
        "--browser-fixtures-dir",
        fixturesDir,
        "--browser-fallback",
        "on-fetch-failure",
        "--budget",
        "1",
        "--json",
    ]);
    assert.equal(result.code, 0);
    const payload = JSON.parse(result.stdout);
    assert.equal(payload.committedPages.length, 1);
    const events = payload.report.events.events ?? payload.report.events;
    assert.ok(
        events.some((event: { kind?: string }) => event.kind === "browser_fallback"),
    );
});

test("crawl pause and resume through checkpoint file", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pandark-cli-"));
    const fixturesDir = join(dir, "fixtures");
    const checkpointPath = join(dir, "checkpoint.json");
    await mkdir(fixturesDir);
    const seed = "http://127.0.0.1:1/pandark-private";
    await writeFile(
        join(fixturesDir, "challenge.snapshot.json"),
        JSON.stringify({
            requested_url: seed,
            final_url: "http://127.0.0.1:1/login",
            document_html: "<html><body>challenge</body></html>",
            captured_at_epoch: 1,
            browser_engine: "fixture",
            profile_id: "test",
            challenge_state: "challenge_required",
        }),
        "utf8",
    );

    const paused = await runCli([
        "crawl",
        seed,
        "--browser-fixtures-dir",
        fixturesDir,
        "--browser-fallback",
        "on-fetch-failure",
        "--challenge-policy",
        "pause-for-operator",
        "--checkpoint-out",
        checkpointPath,
        "--budget",
        "1",
        "--json",
    ]);
    assert.equal(paused.code, 3);
    const pausedPayload = JSON.parse(paused.stdout);
    assert.ok(pausedPayload.checkpoint);
    assert.equal(pausedPayload.report.paused, 1);

    await writeFile(
        join(fixturesDir, "ready.snapshot.json"),
        JSON.stringify({
            requested_url: seed,
            final_url: seed,
            document_html:
                "<html><head><title>Ready</title></head><body><p>ok</p></body></html>",
            captured_at_epoch: 2,
            browser_engine: "fixture",
            profile_id: "test",
            challenge_state: "normal",
        }),
        "utf8",
    );

    const resumed = await runCli([
        "resume",
        checkpointPath,
        "--browser-fixtures-dir",
        fixturesDir,
        "--browser-fallback",
        "on-fetch-failure",
        "--json",
    ]);
    assert.equal(resumed.code, 0);
    const resumedPayload = JSON.parse(resumed.stdout);
    assert.equal(resumedPayload.committedPages.length, 1);
});

test("crawl writes checkpoint into checkpoint-dir on pause", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pandark-cli-"));
    const fixturesDir = join(dir, "fixtures");
    const checkpointDir = join(dir, "checkpoints");
    await mkdir(fixturesDir);
    const seed = "http://127.0.0.1:1/pandark-private-dir";
    await writeFile(
        join(fixturesDir, "challenge.snapshot.json"),
        JSON.stringify({
            requested_url: seed,
            final_url: "http://127.0.0.1:1/login",
            document_html: "<html><body>challenge</body></html>",
            captured_at_epoch: 1,
            browser_engine: "fixture",
            profile_id: "test",
            challenge_state: "challenge_required",
        }),
        "utf8",
    );

    const paused = await runCli([
        "crawl",
        seed,
        "--browser-fixtures-dir",
        fixturesDir,
        "--browser-fallback",
        "on-fetch-failure",
        "--challenge-policy",
        "pause-for-operator",
        "--checkpoint-dir",
        checkpointDir,
        "--budget",
        "1",
        "--json",
    ]);
    assert.equal(paused.code, 3);
    const pausedPayload = JSON.parse(paused.stdout);
    assert.ok(pausedPayload.checkpoint);
    assert.ok(pausedPayload.checkpointPath);
    assert.match(pausedPayload.checkpointPath, /pandark-checkpoint-.*\.json$/);
});

test("crawl reads newline-delimited seed file", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pandark-cli-"));
    const fixturesDir = join(dir, "fixtures");
    await mkdir(fixturesDir);
    const seedA = "http://127.0.0.1:1/pandark-seed-a";
    const seedB = "http://127.0.0.1:1/pandark-seed-b";
    const seedsPath = join(dir, "seeds.txt");
    await writeFile(seedsPath, `# batch\n${seedA}\n${seedB}\n`, "utf8");
    for (const [name, seed] of [
        ["a.snapshot.json", seedA],
        ["b.snapshot.json", seedB],
    ] as const) {
        await writeFile(
            join(fixturesDir, name),
            JSON.stringify({
                requested_url: seed,
                final_url: seed,
                document_html:
                    "<html><head><title>Fixture</title></head><body><p>offline</p></body></html>",
                captured_at_epoch: 1,
                browser_engine: "fixture",
                profile_id: "test",
                challenge_state: "normal",
            }),
            "utf8",
        );
    }

    const result = await runCli([
        "crawl",
        seedsPath,
        "--browser-fixtures-dir",
        fixturesDir,
        "--browser-fallback",
        "on-fetch-failure",
        "--budget",
        "2",
        "--json",
    ]);
    assert.equal(result.code, 0);
    const payload = JSON.parse(result.stdout);
    assert.equal(payload.committedPages.length, 2);
});
