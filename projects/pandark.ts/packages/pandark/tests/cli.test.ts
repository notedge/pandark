import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawn } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { loadPandarkNode } from "../src/node/load.js";

const packageRoot = fileURLToPath(new URL("..", import.meta.url));
const cliEntry = join(packageRoot, "bin/pandark");

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
    // HTML import always records `import.html.partial_coverage`: CSS layout,
    // scripts, forms and arbitrary DOM semantics are not representable in the
    // document graph, so the extract status is partial (exit code 3) with a
    // loss marker, never `complete`.
    assert.equal(result.code, 3);
    const payload = JSON.parse(result.stdout);
    assert.equal(payload.status, "partial");
    assert.equal(payload.report.loss_count, 1);
    assert.ok(payload.document);
    assert.equal(payload.document.coverage.complete, false);
    assert.equal(payload.document.coverage.loss[0].code, "import.html.partial_coverage");
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
    // Same partial-coverage contract as the snapshot case: a plain HTML page
    // still drops CSS layout, scripts, forms and arbitrary DOM semantics, so
    // extraction reports partial coverage instead of claiming `complete`.
    assert.equal(result.code, 3);
    const payload = JSON.parse(result.stdout);
    assert.equal(payload.status, "partial");
    assert.equal(payload.report.loss_count, 1);
    assert.ok(payload.document);
    assert.equal(payload.document.coverage.complete, false);
    assert.equal(payload.document.coverage.loss[0].code, "import.html.partial_coverage");
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

test("crawl pause and resume through a store run id", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pandark-cli-"));
    // Both snapshots key the same seed URL, so the challenge and ready
    // snapshots live in separate directories. Fixture loading applies later
    // directories last, while entries inside one directory load in an
    // unspecified order.
    const challengeDir = join(dir, "challenge");
    const readyDir = join(dir, "ready");
    const storePath = join(dir, "run.yydx");
    await mkdir(challengeDir);
    await mkdir(readyDir);
    const seed = "http://127.0.0.1:1/pandark-private";
    await writeFile(
        join(challengeDir, "challenge.snapshot.json"),
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
        challengeDir,
        "--browser-fallback",
        "on-fetch-failure",
        "--challenge-policy",
        "pause-for-operator",
        "--store",
        storePath,
        "--budget",
        "1",
        "--json",
    ]);
    assert.equal(paused.code, 3);
    const pausedPayload = JSON.parse(paused.stdout);
    assert.equal(pausedPayload.report.paused, 1);
    assert.equal(typeof pausedPayload.runId, "string");

    await writeFile(
        join(readyDir, "ready.snapshot.json"),
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
        pausedPayload.runId,
        "--browser-fixtures-dir",
        `${challengeDir},${readyDir}`,
        "--browser-fallback",
        "on-fetch-failure",
        "--store",
        storePath,
        "--json",
    ]);
    assert.equal(resumed.code, 0);
    const resumedPayload = JSON.parse(resumed.stdout);
    assert.equal(resumedPayload.committedPages.length, 1);
    assert.equal(resumedPayload.runId, pausedPayload.runId);
});

test("resume rejects a missing store path", async () => {
    const result = await runCli(["resume", "run-id-without-store", "--json"]);
    assert.equal(result.code, 1);
});

test("crawl --store and native crawlFile produce the same report", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pandark-cli-"));
    const fixturesDir = join(dir, "fixtures");
    await mkdir(fixturesDir);
    const seed = "http://127.0.0.1:1/pandark-store-parity";
    await writeFile(
        join(fixturesDir, "page.snapshot.json"),
        JSON.stringify({
            requested_url: seed,
            final_url: seed,
            document_html:
                "<html><head><title>Store parity</title></head><body><p>same report</p></body></html>",
            captured_at_epoch: 1,
            browser_engine: "fixture",
            profile_id: "test",
            challenge_state: "normal",
        }),
        "utf8",
    );

    const cliStore = join(dir, "cli.yydx");
    const nativeStore = join(dir, "native.yydx");
    const cliResult = await runCli([
        "crawl",
        seed,
        "--browser-fixtures-dir",
        fixturesDir,
        "--browser-fallback",
        "on-fetch-failure",
        "--budget",
        "1",
        "--store",
        cliStore,
        "--json",
    ]);
    assert.equal(cliResult.code, 0, cliResult.stderr);
    const cliPayload = JSON.parse(cliResult.stdout);

    const binding = loadPandarkNode();
    const nativeResult = await binding.crawlFile(
        seed,
        undefined,
        1,
        fixturesDir,
        "on-fetch-failure",
        undefined,
        undefined,
        undefined,
        undefined,
        undefined,
        nativeStore,
    );
    assert.equal(nativeResult.exitCode, 0);
    assert.deepEqual(JSON.parse(nativeResult.reportJson), cliPayload.report);
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

// ---------------------------------------------------------------------------
// Rust / N-API 抽取报告 parity
// ---------------------------------------------------------------------------

test("inspect html report matches the native binding report", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pandark-cli-"));
    const input = join(dir, "page.html");
    await writeFile(
        input,
        '<html><head><title>Parity</title></head><body><a href="/docs">docs</a><img src="/p.png" alt="p"></body></html>',
        "utf8",
    );

    const cliResult = await runCli(["inspect", input, "--json"]);
    assert.equal(cliResult.code, 0, cliResult.stderr);
    const cliPayload = JSON.parse(cliResult.stdout);

    const binding = loadPandarkNode();
    const native = await binding.inspectInput(input);
    assert.equal(cliPayload.exitCode, native.exitCode);
    assert.deepEqual(cliPayload.report, JSON.parse(native.reportJson));
});

test("inspect login snapshot report matches the native binding report", async () => {
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

    const cliResult = await runCli([
        "inspect",
        input,
        "--from",
        "snapshot",
        "--challenge-policy",
        "stop",
        "--json",
    ]);
    assert.equal(cliResult.code, 2, cliResult.stderr);
    const cliPayload = JSON.parse(cliResult.stdout);

    const binding = loadPandarkNode();
    const native = await binding.inspectInput(input, undefined, "snapshot", "stop");
    assert.equal(cliPayload.exitCode, native.exitCode);
    assert.deepEqual(cliPayload.report, JSON.parse(native.reportJson));
});

test("extract html document and report match the native binding", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pandark-cli-"));
    const input = join(dir, "page.html");
    const documentPath = join(dir, "page.json");
    await writeFile(
        input,
        "<html><head><title>T</title></head><body><p>hello</p><img src='/pic.png' alt='pic'></body></html>",
        "utf8",
    );

    const cliResult = await runCli(["extract", input, "-o", documentPath, "--json"]);
    assert.equal(cliResult.code, 3, cliResult.stderr);
    const cliPayload = JSON.parse(cliResult.stdout);
    const writtenDocument = JSON.parse(await readFile(documentPath, "utf8"));

    const binding = loadPandarkNode();
    const native = await binding.extractInput(input);
    assert.equal(cliPayload.exitCode, native.exitCode);
    assert.equal(cliPayload.status, native.status);
    assert.deepEqual(cliPayload.report, JSON.parse(native.reportJson));
    assert.deepEqual(cliPayload.document, JSON.parse(native.documentJson ?? "null"));
    assert.deepEqual(writtenDocument, cliPayload.document);
});

test("extract snapshot under a challenge policy matches the native binding", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pandark-cli-"));
    const input = join(dir, "challenge.snapshot.json");
    await writeFile(
        input,
        JSON.stringify({
            requested_url: "https://example.com/private",
            final_url: "https://example.com/verify",
            document_html: "<html><head><title>Verify</title></head><body>verification</body></html>",
            captured_at_epoch: 2,
            browser_engine: "fixture",
            profile_id: "test",
            challenge_state: "challenge_required",
        }),
        "utf8",
    );

    const cliResult = await runCli([
        "extract",
        input,
        "--from",
        "snapshot",
        "--challenge-policy",
        "skip-page",
        "--json",
    ]);
    const cliPayload = JSON.parse(cliResult.stdout);

    const binding = loadPandarkNode();
    const native = await binding.extractInput(input, "snapshot", undefined, "skip-page");
    assert.equal(cliPayload.exitCode, native.exitCode);
    assert.equal(cliPayload.status, native.status);
    assert.deepEqual(cliPayload.report, JSON.parse(native.reportJson));
    assert.deepEqual(cliPayload.document, JSON.parse(native.documentJson ?? "null"));
});

test("extractToMarkdown loss codes match the extract document report", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pandark-cli-"));
    const input = join(dir, "merged.html");
    const html =
        "<html><head><title>Merged</title></head><body>" +
        '<table><tr><td colspan="2">merged</td></tr><tr><td>a</td><td>b</td></tr></table>' +
        "</body></html>";
    await writeFile(input, html, "utf8");

    const binding = loadPandarkNode();
    const markdown = await binding.extractToMarkdown(
        new TextEncoder().encode(html),
        "https://example.com/merged",
    );
    // The CLI has no source-url flag, so the native entry must resolve the same
    // `file://` source to keep the report field-by-field comparable.
    const native = await binding.extractFile(input);
    const document = JSON.parse(native.documentJson ?? "null");
    const expectedLossCodes = ["import.html.table_spans", "import.html.partial_coverage"];
    const lossCodes = document.coverage.loss.map((loss: { code: string }) => loss.code);

    assert.equal(markdown.diagnosticNamespace, "notedown.formats.html");
    assert.deepEqual(markdown.diagnosticCodes, expectedLossCodes);
    assert.deepEqual(lossCodes, expectedLossCodes);
    // Merged cells are flattened into a pipe table with a trailing empty column.
    assert.ok(markdown.markdown.includes("| merged |  |"), "merged cells flatten into a pipe table");

    const cliResult = await runCli(["extract", input, "--json"]);
    assert.equal(cliResult.code, native.exitCode, cliResult.stderr);
    const cliPayload = JSON.parse(cliResult.stdout);
    assert.equal(cliPayload.status, native.status);
    assert.deepEqual(cliPayload.report, JSON.parse(native.reportJson));
    assert.deepEqual(
        cliPayload.document.coverage.loss.map((loss: { code: string }) => loss.code),
        expectedLossCodes,
    );
});

test("nested table loss codes match across the native extraction entries", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pandark-cli-"));
    const input = join(dir, "nested.html");
    const html =
        "<html><head><title>Nested</title></head><body>" +
        "<table><tr><td>outer<table><tr><td>inner</td></tr></table></td></tr></table>" +
        "</body></html>";
    await writeFile(input, html, "utf8");

    const binding = loadPandarkNode();
    const markdown = await binding.extractToMarkdown(
        new TextEncoder().encode(html),
        "https://example.com/nested",
    );
    const native = await binding.extractFile(input);
    const document = JSON.parse(native.documentJson ?? "null");
    const lossCodes = document.coverage.loss.map((loss: { code: string }) => loss.code);
    const expectedLossCodes = [
        "import.html.table_nested",
        "import.html.table_cell_blocks",
        "import.html.partial_coverage",
    ];

    assert.equal(markdown.diagnosticNamespace, "notedown.formats.html");
    assert.deepEqual(markdown.diagnosticCodes, expectedLossCodes);
    assert.deepEqual(lossCodes, expectedLossCodes);

    const cliResult = await runCli(["extract", input, "--json"]);
    assert.equal(cliResult.code, native.exitCode, cliResult.stderr);
    const cliPayload = JSON.parse(cliResult.stdout);
    assert.equal(cliPayload.status, native.status);
    assert.deepEqual(cliPayload.report, JSON.parse(native.reportJson));
    assert.deepEqual(
        cliPayload.document.coverage.loss.map((loss: { code: string }) => loss.code),
        expectedLossCodes,
    );
});
