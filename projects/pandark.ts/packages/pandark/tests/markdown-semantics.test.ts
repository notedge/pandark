import assert from "node:assert/strict";
import { createServer } from "node:http";
import { mkdtemp, writeFile } from "node:fs/promises";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { Crawler, defineExtractionProfile } from "../src/index.js";
import { loadPandarkNode } from "../src/node/load.js";

const RICH_HTML = [
    "<!DOCTYPE html><html><head><title>Semantics</title></head><body>",
    "<h1>Main Heading</h1>",
    '<p>Text with <strong>bold</strong> and <em>emphasis</em> plus <a href="https://example.com/x">a link</a>.</p>',
    "<h2>Sub Heading</h2>",
    "<ul><li>alpha</li><li>beta</li></ul>",
    "<ol><li>first</li><li>second</li></ol>",
    "<blockquote>quoted words</blockquote>",
    "<pre><code>let x = 1;</code></pre>",
    '<p><img src="/pic.png" alt="pic alt"></p>',
    "</body></html>",
].join("");

test("extractToMarkdown preserves authored semantics across the HTML to Markdown conversion", { timeout: 60_000 }, async () => {
    const binding = loadPandarkNode();
    const result = await binding.extractToMarkdown(new TextEncoder().encode(RICH_HTML), "https://example.com/semantics");

    assert.equal(result.diagnosticNamespace, "notedown.formats.html");
    const markdown = result.markdown;
    assert.match(markdown, /^# Main Heading/m, "h1 must keep its marker");
    assert.match(markdown, /^## Sub Heading/m, "h2 must keep its marker");
    assert.match(markdown, /\*\*bold\*\*/, "strong must become markdown bold");
    assert.match(markdown, /\*emphasis\*/, "em must become markdown emphasis");
    assert.match(markdown, /\[a link\]\(https:\/\/example\.com\/x\)/, "links must keep text and href");
    assert.match(markdown, /^- alpha$/m, "unordered items must keep list markers");
    assert.match(markdown, /^1\. first$/m, "ordered items must keep their numbers");
    assert.match(markdown, /^2\. second$/m, "ordered numbering must advance");
    assert.match(markdown, /^> quoted words$/m, "blockquotes must keep their marker");
    assert.match(markdown, /```\nlet x = 1;\n```/, "code blocks must keep their content in a fence");
    assert.match(markdown, /!\[pic alt\]\(\/pic\.png\)/, "images must keep alt and src");
});

test("extractWithProfile delivers scoped results through the async entry", async () => {
    const html = [
        "<!DOCTYPE html><html><head><title>Scoped</title></head><body>",
        "<nav>noise</nav>",
        '<article><h1>Article head</h1><p>Article body.</p><a href="/next">next page</a></article>',
        "</body></html>",
    ].join("");
    const server = createServer((_request, response) => {
        response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
        response.end(html);
    });
    await new Promise<void>((resolve, reject) => {
        server.once("error", reject);
        server.listen(0, "127.0.0.1", resolve);
    });
    const { port } = server.address() as AddressInfo;
    const origin = `http://127.0.0.1:${port}`;

    try {
        const crawler = new Crawler();
        const profile = defineExtractionProfile({
            name: "semantics-scope",
            scope: { candidates: ["article"], policy: "first-match" },
            exclude: ["nav"],
        });
        const pending = crawler.extractWithProfile(origin, profile);
        assert.equal(typeof pending.then, "function", "extractWithProfile must stay asynchronous");
        const result = await pending;

        assert.equal(result.scopeStatus, "matched", `scope must match the article: ${JSON.stringify(result.diagnostics)}`);
        assert.ok(result.bodyHtml?.includes("Article body."), "the scoped body must keep the article content");
        assert.ok(!result.bodyHtml?.includes("noise"), "excluded nav must stay out of the scoped body");
        assert.ok(result.links.some((link) => link.href.endsWith("/next")), "links inside the scope must be discovered");
    } finally {
        await new Promise<void>((resolve) => server.close(() => resolve()));
    }
});

test("extractWithProfile reports fetch failure as no-match with a structured diagnostic", async () => {
    const crawler = new Crawler();
    const profile = defineExtractionProfile({
        name: "unreachable",
        scope: { candidates: ["article"], policy: "first-match" },
    });

    // Port 1 is unreachable: the fetch stage must fail without throwing.
    const result = await crawler.extractWithProfile("http://127.0.0.1:1/unreachable", profile);

    assert.equal(result.scopeStatus, "no-match");
    assert.equal(result.bodyHtml, undefined);
    const diagnostic = result.diagnostics.find((d) => d.code === "pandark.extraction.fetch-failed");
    assert.ok(diagnostic, "fetch failure must surface as a structured diagnostic");
    assert.equal(diagnostic?.severity, "error");
});

test("extractInput blocks login snapshots under the stop challenge policy", async () => {
    const dir = await mkdtemp(join(tmpdir(), "pandark-extract-status-"));
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

    const binding = loadPandarkNode();
    const response = await binding.extractInput(input, "snapshot", undefined, "stop");

    assert.equal(response.exitCode, 2, "a blocked challenge must exit with the blocked code");
    assert.equal(response.status, "challenge_blocked");
    assert.equal(response.documentJson, undefined, "no document may be produced from a blocked page");
    const report = JSON.parse(response.reportJson);
    assert.equal(report.challenge_state, "login_required");
});
