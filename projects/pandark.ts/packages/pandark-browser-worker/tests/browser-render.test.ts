import assert from "node:assert/strict";
import { createServer, type Server } from "node:http";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { loadPandarkNode } from "@notedge/pandark";

import { BROWSER_LIMITS, createBrowserWorkerServer } from "../src/index.js";

const packageRoot = fileURLToPath(new URL("..", import.meta.url));
const fixturesDir = join(packageRoot, "fixtures");
const evidenceDir = join(packageRoot, "evidence");
const evidencePath = join(evidenceDir, "browser-render-evidence.json");

interface FixtureServer {
    origin: string;
    hits: Map<string, number>;
    close: () => Promise<void>;
}

/** Serve fixtures plus the synthetic pages the guard checks and fallback need. */
async function startFixtureServer(): Promise<FixtureServer> {
    const hits = new Map<string, number>();
    const server: Server = createServer(async (request, response) => {
        const url = new URL(request.url ?? "/", "http://fixture.local");
        const pathname = url.pathname;
        hits.set(pathname, (hits.get(pathname) ?? 0) + 1);
        if (pathname === "/robots.txt") {
            response.writeHead(200, { "content-type": "text/plain; charset=utf-8" });
            response.end("User-agent: *\nAllow: /\n");
            return;
        }
        if (pathname === "/hang") {
            const timer = setTimeout(() => {
                response.destroy();
            }, 60_000);
            timer.unref();
            return;
        }
        if (pathname === "/redirect") {
            const remaining = Number(url.searchParams.get("n") ?? 0);
            if (remaining > 0) {
                response.writeHead(302, { location: `/redirect?n=${remaining - 1}` });
                response.end();
                return;
            }
            response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
            response.end("<!DOCTYPE html><html><head><title>redirect target</title></head><body><p>done</p></body></html>");
            return;
        }
        if (pathname === "/deep") {
            const levels = 200;
            const body = `<!DOCTYPE html><html><head><title>deep</title></head><body>${"<div>".repeat(levels)}deep${"</div>".repeat(levels)}</body></html>`;
            response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
            response.end(body);
            return;
        }
        if (pathname === "/fat-ssr") {
            const filler = "x".repeat(9 * 1024 * 1024);
            const body = `<!DOCTYPE html><html><head><title>fat</title></head><body><p>${filler}</p></body></html>`;
            response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
            response.end(body);
            return;
        }
        if (pathname.endsWith(".html") || pathname.endsWith(".json")) {
            try {
                const body = await readFile(join(fixturesDir, pathname.replace(/^\//, "")));
                const type = pathname.endsWith(".json") ? "application/json; charset=utf-8" : "text/html; charset=utf-8";
                response.writeHead(200, { "content-type": type });
                response.end(body);
                return;
            } catch {
                // Fall through to 404.
            }
        }
        response.writeHead(404, { "content-type": "text/plain; charset=utf-8" });
        response.end("not found");
    });

    await new Promise<void>((resolve, reject) => {
        server.once("error", reject);
        server.listen(0, "127.0.0.1", resolve);
    });
    const { port } = server.address() as AddressInfo;
    return {
        origin: `http://127.0.0.1:${port}`,
        hits,
        close: () =>
            new Promise<void>((resolve) => {
                server.close(() => resolve());
            }),
    };
}

/** Start a worker on an ephemeral port. */
async function startWorker(options: {
    allowPrivate?: boolean;
    timeoutMs?: number;
}): Promise<{ origin: string; close: () => Promise<void> }> {
    const worker = createBrowserWorkerServer(options);
    await new Promise<void>((resolve, reject) => {
        worker.server.once("error", reject);
        worker.server.listen(0, "127.0.0.1", resolve);
    });
    const { port } = worker.server.address() as AddressInfo;
    return { origin: `http://127.0.0.1:${port}`, close: worker.close };
}

interface WorkerResponse {
    ok: boolean;
    status: number;
    code: string;
    body: unknown;
}

/** Request one snapshot through the worker endpoint contract. */
async function requestSnapshot(origin: string, pageUrl: string): Promise<WorkerResponse> {
    const response = await fetch(`${origin}/?url=${encodeURIComponent(pageUrl)}`);
    const text = await response.text();
    let body: unknown = text;
    try {
        body = JSON.parse(text);
    } catch {
        // Keep the raw text.
    }
    return {
        ok: response.ok,
        status: response.status,
        code: response.headers.get("x-pandark-code") ?? "",
        body,
    };
}

interface Metrics {
    title: string;
    textChars: number;
    blockNodes: number;
    text: string;
}

/** Measure the extraction output without adding a second IR shape. */
function measure(documentJson: string): Metrics {
    const document = JSON.parse(documentJson) as {
        metadata?: { title?: unknown };
        [key: string]: unknown;
    };
    let textChars = 0;
    const parts: string[] = [];
    let blockNodes = 0;
    const walk = (value: unknown): void => {
        if (Array.isArray(value)) {
            for (const item of value) walk(item);
            return;
        }
        if (!value || typeof value !== "object") return;
        const record = value as Record<string, unknown>;
        if (record.block && typeof record.block === "object") blockNodes += 1;
        for (const [key, child] of Object.entries(record)) {
            if (key === "text" && typeof child === "string") {
                textChars += child.length;
                parts.push(child);
                continue;
            }
            walk(child);
        }
    };
    walk(document);
    return {
        title: typeof document.metadata?.title === "string" ? document.metadata.title : "",
        textChars,
        blockNodes,
        text: parts.join("\n"),
    };
}

interface ModeResult extends Metrics {
    ok: boolean;
    extractionStatus?: string;
    failure?: { code: string, message: string };
}

/** HTTP direct path: crawl one page, then extract the raw body. */
async function directExtract(binding: ReturnType<typeof loadPandarkNode>, pageUrl: string): Promise<ModeResult> {
    try {
        const crawl = await binding.crawlFile(pageUrl, 0, 1);
        const page = crawl.pages?.[0];
        if (!page) {
            return {
                ok: false,
                title: "",
                textChars: 0,
                blockNodes: 0,
                text: "",
                failure: { code: "direct.no-page", message: "crawl returned no page" },
            };
        }
        const dir = await mkdtemp(join(tmpdir(), "pandark-direct-"));
        const input = join(dir, "page.html");
        await writeFile(input, page.body);
        const extracted = await binding.extractInput(input, "html", pageUrl, "stop");
        const metrics = measure(extracted.documentJson ?? "{}");
        return { ok: true, extractionStatus: extracted.status, ...metrics };
    } catch (error) {
        return {
            ok: false,
            title: "",
            textChars: 0,
            blockNodes: 0,
            text: "",
            failure: { code: "direct.failed", message: error instanceof Error ? error.message : String(error) },
        };
    }
}

/** Render path: worker snapshot, then the same native extract with `from: snapshot`. */
async function renderedExtract(
    binding: ReturnType<typeof loadPandarkNode>,
    workerOrigin: string,
    pageUrl: string,
): Promise<ModeResult> {
    const snapshot = await requestSnapshot(workerOrigin, pageUrl);
    if (!snapshot.ok) {
        const payload = snapshot.body as { code?: string, message?: string };
        return {
            ok: false,
            title: "",
            textChars: 0,
            blockNodes: 0,
            text: "",
            failure: { code: payload.code ?? snapshot.code, message: payload.message ?? "worker rejected the capture" },
        };
    }
    const dir = await mkdtemp(join(tmpdir(), "pandark-render-"));
    const input = join(dir, "snapshot.json");
    await writeFile(input, JSON.stringify(snapshot.body));
    const extracted = await binding.extractInput(input, "snapshot", pageUrl, "stop");
    const metrics = measure(extracted.documentJson ?? "{}");
    return { ok: true, extractionStatus: extracted.status, ...metrics };
}

interface Scenario {
    name: string;
    path: string;
    expectedRenderedTitle: string;
    bodyMarker: string;
}

const SCENARIOS: Scenario[] = [
    {
        name: "static-article",
        path: "/static-article.html",
        expectedRenderedTitle: "静态文章样本",
        bodyMarker: "静态正文标记：直取即可抽取。",
    },
    {
        name: "js-shell",
        path: "/js-shell.html",
        expectedRenderedTitle: "浏览器渲染文章样本",
        bodyMarker: "渲染正文标记：脚本注入后才会出现。",
    },
    {
        name: "js-fetch",
        path: "/js-fetch.html",
        expectedRenderedTitle: "JS 渲染列表样本",
        bodyMarker: "列表正文标记：fetch 返回后才会出现。",
    },
];

/** Keep the mode record stable: metrics plus marker hit, no temp paths. */
function modeRecord(result: ModeResult, marker: string): Record<string, unknown> {
    const record: Record<string, unknown> = {
        ok: result.ok,
        title: result.title,
        text_chars: result.textChars,
        block_nodes: result.blockNodes,
        body_marker_hit: result.text.includes(marker),
    };
    if (result.extractionStatus) record.extraction_status = result.extractionStatus;
    if (result.failure) record.failure = result.failure;
    return record;
}

test("direct fetch and Playwright render produce comparable extraction evidence", { timeout: 180_000 }, async () => {
    const binding = loadPandarkNode();
    const fixtures = await startFixtureServer();
    const worker = await startWorker({ allowPrivate: true });
    const strictWorker = await startWorker({ allowPrivate: false });
    const shortWorker = await startWorker({ allowPrivate: true, timeoutMs: 2_000 });
    const fixturesEvidence: Array<Record<string, unknown>> = [];
    const guards: Record<string, unknown> = {};

    try {
        const engineSnapshot = await requestSnapshot(worker.origin, `${fixtures.origin}/static-article.html`);
        assert.equal(engineSnapshot.ok, true, "the static fixture must capture");
        const engine = engineSnapshot.body as { browser_engine?: string, profile_id?: string, challenge_state?: string };
        assert.equal(engine.challenge_state, "normal", "the static fixture must settle as a normal snapshot");
        assert.ok(engine.browser_engine?.startsWith("chromium-"), "the snapshot must name the bundled Chromium");

        for (const scenario of SCENARIOS) {
            const pageUrl = `${fixtures.origin}${scenario.path}`;
            const direct = await directExtract(binding, pageUrl);
            const rendered = await renderedExtract(binding, worker.origin, pageUrl);
            assert.equal(rendered.ok, true, `${scenario.name} must render: ${JSON.stringify(rendered.failure)}`);
            assert.equal(rendered.title, scenario.expectedRenderedTitle, `${scenario.name} rendered title`);
            assert.ok(rendered.textChars > 0, `${scenario.name} rendered text must be non-empty`);
            assert.equal(rendered.text.includes(scenario.bodyMarker), true, `${scenario.name} rendered body marker`);

            fixturesEvidence.push({
                name: scenario.name,
                path: scenario.path,
                expected_title: scenario.expectedRenderedTitle,
                body_marker: scenario.bodyMarker,
                direct: modeRecord(direct, scenario.bodyMarker),
                rendered: modeRecord(rendered, scenario.bodyMarker),
            });

            if (scenario.name === "js-shell") {
                assert.equal(direct.title, "JS 骨架页面", "direct extraction must see only the script shell title");
                assert.equal(direct.text.includes(scenario.bodyMarker), false, "direct extraction must not reach the injected article");
                assert.ok(direct.textChars < rendered.textChars, "rendered text must exceed the direct skeleton text");
            }
            if (scenario.name === "js-fetch") {
                assert.equal(direct.text.includes(scenario.bodyMarker), false, "direct extraction must not reach the fetched list");
                assert.ok(direct.textChars < rendered.textChars, "fetch-rendered list must beat the direct skeleton");
                assert.ok(rendered.text.includes("夹具丙"), "the fetched JSON list must reach the extracted text");
            }
            if (scenario.name === "static-article") {
                assert.equal(direct.title, "静态文章样本", "static title must be reachable without a browser");
                assert.equal(direct.text.includes(scenario.bodyMarker), true, "static body must be reachable without a browser");
            }
        }

        // Guard: private targets are denied before any browser work starts.
        const privateResponse = await requestSnapshot(strictWorker.origin, `${fixtures.origin}/static-article.html`);
        assert.equal(privateResponse.status, 403);
        guards.private_address_denied = { status: privateResponse.status, code: privateResponse.code };

        // Guard: a page that never responds hits the per-page timeout.
        const timeoutResponse = await requestSnapshot(shortWorker.origin, `${fixtures.origin}/hang`);
        assert.equal(timeoutResponse.status, 504);
        guards.render_timeout = { status: timeoutResponse.status, code: timeoutResponse.code, timeout_ms: 2_000 };

        // Guard: the redirect chain stops at the published hop limit.
        const redirectResponse = await requestSnapshot(worker.origin, `${fixtures.origin}/redirect?n=6`);
        assert.equal(redirectResponse.status, 508);
        guards.redirect_limit = {
            status: redirectResponse.status,
            code: redirectResponse.code,
            max_redirects: BROWSER_LIMITS.maxRedirects,
        };

        // Guard: deeply nested markup is rejected after settling.
        const deepResponse = await requestSnapshot(worker.origin, `${fixtures.origin}/deep`);
        assert.equal(deepResponse.status, 422);
        guards.dom_depth_limit = {
            status: deepResponse.status,
            code: deepResponse.code,
            max_dom_depth: BROWSER_LIMITS.maxDomDepth,
        };

        // Guard: an oversized document fails closed on both sides.
        const fatResponse = await requestSnapshot(worker.origin, `${fixtures.origin}/fat-ssr`);
        assert.equal(fatResponse.status, 413);
        guards.snapshot_too_large = {
            status: fatResponse.status,
            code: fatResponse.code,
            max_snapshot_bytes: BROWSER_LIMITS.maxSnapshotBytes,
        };

        const fatBefore = fixtures.hits.get("/fat-ssr") ?? 0;
        const fallback = await binding.crawlFile(
            `${fixtures.origin}/fat-ssr`,
            0,
            1,
            undefined,
            "on-fetch-failure",
            "stop",
            worker.origin,
        );
        const fatVisits = (fixtures.hits.get("/fat-ssr") ?? 0) - fatBefore;
        const report = JSON.parse(fallback.reportJson) as {
            events?: { events?: Array<{ kind?: string, fetch_error?: string, reason?: string }> };
        };
        const fallbackEvents = (report.events?.events ?? []).filter((event) => event.kind === "browser_fallback");
        const failedEvents = (report.events?.events ?? []).filter((event) => event.kind === "failed");
        assert.equal(fallback.committedPages.length, 0, "the oversized page must not commit");
        assert.equal(fatVisits >= 1, true, "the crawl must reach the page again through the browser worker");
        assert.equal(fallbackEvents.length, 0, "the failing snapshot keeps the fetch failure as the reported reason");
        const browserFallbackEvidence = {
            fetch_error: failedEvents[0]?.reason ?? "",
            worker_code: fatResponse.code,
            crawler_browser_fallback_event: fallbackEvents.length > 0,
            page_visits_during_crawl: fatVisits,
            committed_pages: fallback.committedPages.length,
            note: "the 8 MiB fetch body budget and the 8 MiB snapshot cap both reject this page, so the crawl reports only the fetch failure",
        };

        // Real site comparison. An unreachable network is recorded, never retried.
        const realSiteUrl = "https://quotes.toscrape.com/js/";
        const realSiteMarker = "The world as we have created it is a process";
        let realSite: Record<string, unknown>;
        try {
            const rendered = await renderedExtract(binding, worker.origin, realSiteUrl);
            const direct = await directExtract(binding, realSiteUrl);
            realSite = {
                status: rendered.ok ? "ok" : "blocked",
                url: realSiteUrl,
                body_marker: realSiteMarker,
                direct: modeRecord(direct, realSiteMarker),
                rendered: modeRecord(rendered, realSiteMarker),
            };
            if (rendered.ok) {
                assert.ok(rendered.textChars > 100, "the real JS page must render a filled quote list");
            }
        } catch (error) {
            realSite = {
                status: "blocked",
                url: realSiteUrl,
                reason: error instanceof Error ? error.message : String(error),
            };
        }

        await mkdir(evidenceDir, { recursive: true });
        const evidence = {
            schema: "pandark.browser-render-evidence/v0",
            worker: {
                engine: engine.browser_engine,
                profile_id: engine.profile_id,
                settle: "readyState complete plus 2 animation frames without DOM mutations",
            },
            limits: { ...BROWSER_LIMITS },
            fixtures: fixturesEvidence,
            browser_fallback: browserFallbackEvidence,
            guards,
            real_site: realSite,
        };
        await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`, "utf8");
    } finally {
        await worker.close();
        await strictWorker.close();
        await shortWorker.close();
        await fixtures.close();
    }
});
