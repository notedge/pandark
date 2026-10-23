import assert from "node:assert/strict";
import test from "node:test";

import { Crawler } from "../src/crawler.js";

test("Crawler.fetch() fetches seed page through Rust binding", async () => {
    const crawler = new Crawler();
    // Use a stable, always-reachable URL
    const artifact = await crawler.fetch("https://example.com");
    assert.ok(artifact.finalUrl.length > 0, "should have finalUrl");
    assert.ok(artifact.bodyText.length > 0, "should have body text");
    assert.ok(artifact.bodyBytes.length > 0, "should have body bytes");
    assert.ok(artifact.finalUrl.includes("example.com"), `finalUrl should be example.com: ${artifact.finalUrl}`);
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
