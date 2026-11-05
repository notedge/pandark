import assert from "node:assert/strict";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { loadPandarkNode } from "../src/node/load.js";

const MERMAID_DIV_HTML = [
    "<!DOCTYPE html><html><head><title>Flow</title></head><body>",
    "<article><h1>Pipeline</h1>",
    '<div class="mermaid">',
    "flowchart TD\n  A[Fetch] --&gt; B{OK?}\n  B --&gt;|yes| C[Extract]",
    "</div>",
    "</article></body></html>",
].join("");

const MERMAID_PRE_HTML = [
    "<!DOCTYPE html><html><head><title>Fence</title></head><body>",
    "<article>",
    '<pre><code class="language-mermaid">flowchart LR\n  X --&gt; Y</code></pre>',
    "</article></body></html>",
].join("");

const INLINE_SVG =
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10"/></svg>';

const RENDERED_HTML = [
    "<!DOCTYPE html><html><head><title>Rendered</title></head><body>",
    `<p>before</p>${INLINE_SVG}`,
    '<p><img class="mermaid" src="renders/flow.png" alt="flowchart"></p>',
    "</body></html>",
].join("");

test("mermaid source tier keeps the authored source as a fenced block", { timeout: 60_000 }, async () => {
    const binding = loadPandarkNode();
    for (const [name, html, source] of [
        ["div.mermaid", MERMAID_DIV_HTML, "flowchart TD\n  A[Fetch] --> B{OK?}\n  B -->|yes| C[Extract]"],
        ["pre code.language-mermaid", MERMAID_PRE_HTML, "flowchart LR\n  X --> Y"],
    ] as const) {
        const result = await binding.extractToMarkdown(new TextEncoder().encode(html), "https://example.com/repro");
        assert.ok(
            result.markdown.includes(`\`\`\`mermaid\n${source}\n\`\`\``),
            `${name} must round-trip the authored mermaid source: ${result.markdown}`,
        );
    }
});

test("inline svg and bitmap diagrams keep their resource forms through extraction", { timeout: 60_000 }, async () => {
    const dir = await mkdtemp(join(tmpdir(), "pandark-mermaid-"));
    const input = join(dir, "rendered.html");
    await writeFile(input, RENDERED_HTML, "utf8");

    const binding = loadPandarkNode();
    const response = await binding.extractFile(input, "https://example.com/docs/rendered.html");

    assert.equal(response.status, "partial", `bitmap-only diagrams mark the extract partial: ${response.reportJson}`);
    const document = JSON.parse(response.documentJson ?? "");

    // Tier 2: inline svg is a RawHtml block plus a resolved graphic resource asset.
    const rawHtmlBlocks = document.blocks.filter((node: { block: Record<string, unknown> }) => "RawHtml" in node.block);
    assert.ok(rawHtmlBlocks.length >= 1, "inline svg must survive as RawHtml blocks");
    const svgAsset = document.assets.find(
        (asset: { media_type?: string }) => asset.media_type === "image/svg+xml",
    );
    assert.ok(svgAsset, "inline svg must be registered as an image/svg+xml asset");
    assert.equal(svgAsset.status, "Resolved");
    const payload = new TextDecoder().decode(Uint8Array.from(svgAsset.bytes));
    assert.ok(payload.includes("<svg"), "asset bytes must be the exact svg markup");
    const svgNodeIds = new Set(rawHtmlBlocks.map((node: { id: number }) => node.id));
    assert.ok(
        document.relations.some(
            (relation: { kind: string; source: { Node?: number }; target: { Asset?: number } }) =>
                relation.kind === "Embeds"
                && relation.source.Node !== undefined
                && svgNodeIds.has(relation.source.Node)
                && relation.target.Asset === svgAsset.id,
        ),
        "the RawHtml svg block must embed the registered asset",
    );

    // Tier 3: the bitmap stays an image asset and the IR marks 位图，无结构.
    const bitmapAsset = document.assets.find(
        (asset: { source?: string }) => asset.source === "https://example.com/docs/renders/flow.png",
    );
    assert.ok(bitmapAsset, "the bitmap must resolve against the caller-supplied page url");
    const marker = document.coverage.loss.find(
        (loss: { code: string }) => loss.code === "crawler.extract.diagram_bitmap",
    );
    assert.ok(marker, "bitmap-only diagrams must carry the explicit loss marker");
    assert.ok(marker.message.includes("位图，无结构"), marker.message);
    assert.ok(marker.message.includes("flowchart"), "alt text must be recorded");
});

test("inline svg markup passes through the markdown export verbatim", { timeout: 60_000 }, async () => {
    const binding = loadPandarkNode();
    const html = `<!DOCTYPE html><html><head><title>Svg</title></head><body>${INLINE_SVG}</body></html>`;
    const result = await binding.extractToMarkdown(new TextEncoder().encode(html), "https://example.com/svg");
    assert.ok(result.markdown.includes("<svg"), `svg markup must stay in the markdown output: ${result.markdown}`);
});
