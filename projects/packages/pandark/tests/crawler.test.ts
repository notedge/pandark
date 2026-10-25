import assert from "node:assert/strict";
import { spawn, type ChildProcess } from "node:child_process";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import test from "node:test";

import { Crawler } from "../src/crawler.js";
import { loadPandarkNode } from "../src/node/load.js";

test("Crawler.fetch() awaits the async Rust binding and returns the artifact", async () => {
    const crawler = new Crawler();
    // Use a stable, always-reachable URL
    const pending = crawler.fetch("https://example.com");
    assert.equal(typeof pending.then, "function", "fetch must return the binding Promise, not a synchronous artifact");
    const artifact = await pending;
    assert.ok(artifact.finalUrl.length > 0, "should have finalUrl");
    assert.ok(artifact.bodyText.length > 0, "should have body text");
    assert.ok(artifact.bodyBytes.length > 0, "should have body bytes");
    assert.ok(artifact.finalUrl.includes("example.com"), `finalUrl should be example.com: ${artifact.finalUrl}`);
});

test("fetchSeed and crawlFile bindings resolve without blocking the event loop", { timeout: 20_000 }, async () => {
    const server = createServer((request, response) => {
        if ((request.url ?? "/") === "/") {
            response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
            response.end("<html><body><p>fixture</p></body></html>");
            return;
        }
        response.writeHead(404, { "content-type": "text/plain" });
        response.end("not found");
    });
    await new Promise<void>((resolve, reject) => {
        server.once("error", reject);
        server.listen(0, "127.0.0.1", resolve);
    });
    const { port } = server.address() as AddressInfo;
    const origin = `http://127.0.0.1:${port}`;
    const binding = loadPandarkNode();

    // The fixture server runs on this event loop: the requests can only be
    // answered while the crawler awaits its binding. A blocking binding would
    // stall the server and this test would hit its timeout.
    let ticks = 0;
    let running = true;
    const tick = (): void => {
        ticks += 1;
        if (running) setImmediate(tick);
    };
    setImmediate(tick);

    try {
        const pendingFetch = binding.fetchSeed(origin);
        assert.equal(typeof pendingFetch.then, "function", "fetchSeed must return a Promise");
        const fetchResponse = await pendingFetch;
        const artifact = JSON.parse(fetchResponse.artifactJson) as {
            final_url?: string;
            body?: { inline?: number[] };
        };
        assert.ok((artifact.final_url ?? "").startsWith(origin), "fetchSeed should return the fixture URL");
        const body = new TextDecoder().decode(new Uint8Array(artifact.body?.inline ?? []));
        assert.ok(body.includes("fixture"), "fetchSeed should return the fixture body");

        const pendingCrawl = binding.crawlFile(origin, 0, 1);
        assert.equal(typeof pendingCrawl.then, "function", "crawlFile must return a Promise");
        const crawlResponse = await pendingCrawl;
        assert.equal(crawlResponse.committedPages.length, 1, "crawlFile should commit the seed page");
    }
    finally {
        running = false;
        await new Promise<void>((resolve) => server.close(() => resolve()));
    }

    assert.ok(ticks > 0, "the event loop must keep running while the bindings await");
});

test("Crawler.crawl() fetches seed and invokes handlePage with real page content", async () => {
    const crawler = new Crawler();

    let pageSeen = false;
    let pageUrl = "";
    let pageBody = "";

    const run = await crawler.crawl("https://example.com", {
        handlePage: async (page) => {
            pageSeen = true;
            pageUrl = page.url;
            pageBody = await page.html();
            return { text: { processed: true } };
        },
    });

    assert.ok(pageSeen, "handlePage should have been called");
    assert.ok(pageUrl.includes("example.com"), `page URL should be example.com: ${pageUrl}`);
    assert.ok(pageBody.length > 0, "page body should have content");
    assert.ok(pageBody.includes("<html") || pageBody.includes("<!doctype"), "page body should look like HTML");

    // Run-level assertions — should NOT be success_unvalidated anymore
    assert.notEqual(run.status, "success_unvalidated",
        "should not be the old noop success_unvalidated — bridge should execute real fetch");
    assert.equal(run.status, "success", "run should succeed");
    assert.equal(run.budgetStatus, "completed", "single-page run is completed");
    assert.equal(run.summary.fetched, 1);
    assert.equal(run.summary.success, 1);
    assert.equal(run.summary.failed, 0);

    const outcomes = run.outcomes;
    assert.equal(outcomes.size, 1);
    const [, outcome] = outcomes.entries().next().value as [string, unknown];
    assert.equal((outcome as { status: string }).status, "success");
    assert.deepEqual((outcome as { output: unknown }).output, { processed: true });
});

test("Crawler.crawl() returns failed status when fetchSeed fails", async () => {
    const crawler = new Crawler();

    // Port 1 guaranteed to be unreachable, Rust transport will fail with connection refused
    const run = await crawler.crawl("http://127.0.0.1:1/never-reachable", {
        handlePage: async (_page) => {
            // Should never be called
            throw new Error("handlePage should not run when fetch fails");
        },
    });

    assert.equal(run.status, "failed", "run should be failed on fetch error");
    assert.equal(run.summary.fetched, 1, "attempted to fetch");
    assert.equal(run.summary.failed, 1);
    assert.equal(run.summary.success, 0);

    // Should have fetch-failed diagnostic
    const hasFetchError = run.diagnostics.some((d) => d.code === "pandark.crawl.fetch-failed");
    assert.ok(hasFetchError, "should have pandark.crawl.fetch-failed diagnostic");
});

test("Crawler.crawl() returns failed status when handlePage throws", async () => {
    const crawler = new Crawler();

    const run = await crawler.crawl("https://example.com", {
        handlePage: async (_page) => {
            throw new Error("intentional handler failure");
        },
    });

    assert.equal(run.status, "failed", "run should be failed on handler error");
    const hasHandlerError = run.diagnostics.some((d) => d.code === "pandark.crawl.handler-threw");
    assert.ok(hasHandlerError, "should have pandark.crawl.handler-threw diagnostic");
});

test("Crawler.crawl() derives success_with_loss from handler warnings", async () => {
    const crawler = new Crawler();

    const run = await crawler.crawl("https://example.com", {
        handlePage: async (_page) => {
            return {
                diagnostics: [{
                    code: "panduck.convert.structure-loss",
                    severity: "warn",
                    messageKey: "structure.loss.missing",
                }],
                text: { partial: true },
            };
        },
    });

    assert.equal(run.status, "success_with_loss", "run should reflect loss warning");
    const [, outcome] = run.outcomes.entries().next().value as [string, unknown];
    assert.equal((outcome as { status: string }).status, "success_with_loss");
});

test("Crawler.crawl() forwards scope to the Rust frontier and delivers each committed page", async () => {
    const site = await startFixtureSite();
    try {
        const handled: string[] = [];
        const crawler = new Crawler();
        const run = await crawler.crawl(site.origin, {
            scope: { allowedHosts: ["127.0.0.1"] },
            limits: { maxDepth: 1, maxRequests: 10 },
            handlePage: async (page) => {
                handled.push(page.url);
                return { text: await page.html() };
            },
        });

        assert.equal(run.status, "success", "scoped crawl should succeed");
        assert.equal(run.budgetStatus, "completed");
        assert.equal(run.remainingFrontier, 0);
        assert.equal(run.summary.scopeRejected, 0, "allowed host must not be rejected");
        assert.equal(run.summary.success, 3, "seed and both linked pages should commit");
        assert.equal(run.outcomes.size, 3);
        assert.equal(handled.length, 3, "handlePage runs for every committed page");
        for (const outcome of run.outcomes.values()) {
            assert.equal(outcome.status, "success");
            assert.ok(
                typeof outcome.output === "string" && (outcome.output as string).includes("<html"),
                `page HTML should reach handlePage output: ${String(outcome.output)}`,
            );
        }
    }
    finally {
        await site.close();
    }
});

test("Crawler.crawl() fails the run when handlePage throws on a committed page", async () => {
    const site = await startFixtureSite();
    try {
        const crawler = new Crawler();
        const run = await crawler.crawl(site.origin, {
            scope: { allowedHosts: ["127.0.0.1"] },
            limits: { maxDepth: 1, maxRequests: 10 },
            handlePage: async (page) => {
                if (new URL(page.url).pathname === "/a") throw new Error("boom");
                return { text: page.url };
            },
        });

        assert.equal(run.status, "failed", "a handler failure must fail the run");
        assert.ok(
            run.diagnostics.some((diagnostic) => diagnostic.code === "pandark.crawl.handler-threw"),
            "handler failure should be in the run ledger",
        );
        const failed = [...run.outcomes.values()].filter((outcome) => outcome.status === "failed");
        assert.equal(failed.length, 1);
        assert.equal(new URL(failed[0].url).pathname, "/a");
    }
    finally {
        await site.close();
    }
});

test("Crawler.crawl() reports scope rejection from the Rust scope gate", async () => {
    const site = await startFixtureSite();
    try {
        let handled = 0;
        const crawler = new Crawler();
        const run = await crawler.crawl(site.origin, {
            scope: { allowedHosts: ["example.invalid"] },
            handlePage: async () => {
                handled += 1;
                return { text: "unused" };
            },
        });

        assert.equal(run.summary.scopeRejected, 1, "seed host is outside the allow-list");
        assert.equal(run.summary.fetched, 0, "out-of-scope seed must not be fetched");
        assert.equal(handled, 0, "rejected pages must not reach handlePage");
        const [outcome] = run.outcomes.values();
        assert.equal(outcome.status, "skipped");
        assert.ok(
            outcome.diagnostics.some((diagnostic) => diagnostic.code === "pandark.scope.rejected"),
            "scope rejection should surface as a diagnostic",
        );
    }
    finally {
        await site.close();
    }
});

test("Crawler.crawl() surfaces budget exhaustion instead of success", async () => {
    const site = await startFixtureSite();
    try {
        let handled = 0;
        const crawler = new Crawler();
        const run = await crawler.crawl(site.origin, {
            scope: { allowedHosts: ["127.0.0.1"] },
            limits: { maxDepth: 1, maxRequests: 1 },
            handlePage: async () => {
                handled += 1;
                return { text: "seed" };
            },
        });

        assert.equal(run.budgetStatus, "exhausted");
        assert.equal(run.status, "success_unvalidated", "budget exhaustion must not masquerade as success");
        assert.ok(run.remainingFrontier >= 2, `both queued links should remain: ${run.remainingFrontier}`);
        assert.equal(run.summary.success, 1, "only the seed was extracted");
        assert.equal(handled, 1, "only committed pages reach handlePage");
        assert.equal([...run.outcomes.values()][0].output, "seed");
        assert.ok(
            run.diagnostics.some((diagnostic) => diagnostic.code === "pandark.crawl.budget-exhausted"),
            "budget exhaustion diagnostic must be present",
        );
    }
    finally {
        await site.close();
    }
});

test("Crawler politeness preset reaches the Rust robots gate", async () => {
    const site = await startFixtureSite({
        "/": "<html><head><title>Home</title></head><body><a href=\"/admin\">admin</a><a href=\"/a\">a</a></body></html>",
        "/a": "<html><head><title>A</title></head><body>page a</body></html>",
        "/admin": "<html><head><title>Admin</title></head><body>admin page</body></html>",
    });
    try {
        const adminUrl = `${site.origin}/admin`;
        const crawlOptions = {
            scope: { allowedHosts: ["127.0.0.1"] },
            limits: { maxDepth: 1, maxRequests: 10 },
        };

        // Conservative (the Rust default) denies the `/admin` fallback path when
        // robots.txt is missing.
        const conservative = new Crawler({ politeness: "conservative" });
        const conservativeRun = await conservative.crawl(site.origin, {
            ...crawlOptions,
            handlePage: async (page) => ({ text: page.url }),
        });
        assert.notEqual(conservativeRun.outcomes.get(adminUrl)?.status, "success",
            "conservative politeness must deny /admin");

        // Developer-level politeness forwards through `crawlFile` and disables
        // the fallback deny list, so `/admin` is fetched.
        const aggressive = new Crawler({ politeness: "aggressive" });
        const aggressiveRun = await aggressive.crawl(site.origin, {
            ...crawlOptions,
            handlePage: async (page) => ({ text: page.url }),
        });
        assert.equal(aggressiveRun.outcomes.get(adminUrl)?.status, "success",
            "aggressive politeness must let /admin through");
    }
    finally {
        await site.close();
    }
});

/**
 * Fixture site served from a child process.
 *
 * The child keeps the fixture event loop independent of the test process, so
 * the fixture server stays reachable throughout a crawl run.
 */
const DEFAULT_FIXTURE_PAGES: Record<string, string> = {
    "/": "<html><head><title>Home</title></head><body><a href=\"/a\">a</a><a href=\"/b\">b</a></body></html>",
    "/a": "<html><head><title>A</title></head><body>page a</body></html>",
    "/b": "<html><head><title>B</title></head><body>page b</body></html>",
};

async function startFixtureSite(pages: Record<string, string> = DEFAULT_FIXTURE_PAGES): Promise<{ process: ChildProcess; origin: string; close: () => Promise<void> }> {
    const script = `
        const http = require("node:http");
        const pages = JSON.parse(process.env.PANDARK_FIXTURE_PAGES);
        const server = http.createServer((request, response) => {
            const path = (request.url || "/").split("?")[0];
            const body = pages[path];
            if (body === undefined) {
                response.writeHead(404, { "content-type": "text/plain" });
                response.end("not found");
                return;
            }
            response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
            response.end(body);
        });
        server.listen(0, "127.0.0.1", () => {
            process.stdout.write(String(server.address().port) + "\\n");
        });
    `;
    const child = spawn(process.execPath, ["-e", script], {
        env: { ...process.env, PANDARK_FIXTURE_PAGES: JSON.stringify(pages) },
        stdio: ["ignore", "pipe", "pipe"],
    });
    const port = await new Promise<number>((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error("fixture server did not start")), 10_000);
        let buffered = "";
        child.stdout?.on("data", (chunk: Buffer) => {
            buffered += chunk.toString();
            const line = buffered.split("\n")[0];
            if (line) {
                clearTimeout(timer);
                resolve(Number.parseInt(line, 10));
            }
        });
        child.once("error", (error) => {
            clearTimeout(timer);
            reject(error);
        });
        child.once("exit", (code) => {
            clearTimeout(timer);
            reject(new Error(`fixture server exited early with code ${code}`));
        });
    });
    return {
        process: child,
        origin: `http://127.0.0.1:${port}`,
        close: () => new Promise<void>((resolve) => {
            child.once("exit", () => resolve());
            child.kill();
        }),
    };
}
