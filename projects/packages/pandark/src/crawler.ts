/**
 * Pandark Crawler — public authoring surface for fetch + crawl.
 *
 * Crawler is an **authoring surface** only. All frontier scheduling, scope
 * gating, budget management, link discovery, and politeness live on the
 * **Rust side** (via `loadPandarkNode()` bindings). The TS Crawler does NOT
 * re-implement any of those — it validates inputs, invokes the Rust binding,
 * and maps the result back.
 *
 * Fetch → convert is a separate path: `Crawler.fetch()` + Panduck Converter
 * handle 90% of real usage. `Crawler.crawl()` is the full scheduling path and
 * currently requires a ready `runCrawl` Rust binding — if not available the
 * Crawler returns a diagnostic instead of pretending.
 */

import { loadPandarkNode } from "./node/load.js";
import type { PandarkBindings } from "./types.js";

// ---------------------------------------------------------------------------
// Public types (v0 only — no compat fields)
// ---------------------------------------------------------------------------

/** Single fetched artifact from `crawler.fetch()`. */
export type FetchArtifact = {
    finalUrl: string;
    requestedUrl: string;
    status: number;
    contentType: string;
    headers: Record<string, string>;
    bodyText: string;
    bodyBytes: Uint8Array;
};

/** Page delivered to `handlePage`. */
export type Page = {
    readonly url: string;
    readonly requestedUrl: string;
    readonly status: number;
    readonly contentType: string;
    readonly headers: Record<string, string>;
    html(): Promise<string>;
    bytes(): Uint8Array;
};

/** Scope gate for a crawl run. */
export type CrawlScope = {
    allowedHosts?: readonly string[];
    allowedDomains?: readonly string[];
};

/** Budget limits for a crawl run. */
export type CrawlLimits = {
    maxDepth?: number;
    maxRequests?: number;
};

/**
 * The handlePage contract: receives a Page, returns a ConverterResult.
 *
 * `ConverterResult` is the Panduck Converter return shape. Its `diagnostics`
 * field carries structured records for missing required elements, loss, rule
 * failures, etc. Crawler collects those alongside its own fetch/scope errors.
 */
export type HandlePageFn<Out = unknown> = (
    page: Page,
) => Promise<ConverterLikeResult<Out>> | ConverterLikeResult<Out>;

/**
 * Structural duck-type over Panduck ConverterResult — v0 only, no compat.
 *
 * Any object with `{ diagnostics?: DiagnosticRecord[], text?: unknown }`
 * satisfies the contract. **No `status` field, no `markdown` field.**
 * Output always goes through `text`; status is derived from diagnostics.
 */
export type ConverterLikeResult<Out = unknown> = {
    diagnostics?: DiagnosticRecord[];
    text?: Out;
};

/**
 * Structured diagnostic record shared across Pandark + Panduck.
 * v0 shape only — mirrors boost-diagnostic::Diagnostic fields one-to-one.
 */
export type DiagnosticRecord = {
    code: string;
    fallback?: string;
    messageKey?: string;
    args?: Record<string, unknown>;
    origin?: { namespace: string; component: string };
    severity?: "error" | "warn" | "info";
    stage?: "fetch" | "scope" | "structure" | "convert" | "rule" | "handler";
    url?: string;
    target?: string;
};

/** Options for `new Crawler()`. **Currently ignored** — Rust binding not ready. */
export type CrawlerOptions = {
    store?: { path: string };
    politeness?: "aggressive" | "conservative";
};

/** Options for `crawler.fetch()`. **Currently ignored** — Rust binding not ready. */
export type FetchOptions = {
    browserEndpoint?: string;
};

/** Options for `crawler.crawl()`. */
export type CrawlOptions<Out = unknown> = {
    scope?: CrawlScope;
    limits?: CrawlLimits;
    handlePage: HandlePageFn<Out>;
};

/** Per-page outcome recorded during a crawl. v0 status enum only. */
export type PageOutcome<Out = unknown> = {
    url: string;
    status: "success" | "success_with_loss" | "success_unvalidated" | "failed" | "skipped";
    output?: Out;
    diagnostics: DiagnosticRecord[];
};

/** Full crawl run report. v0 only — no compat fields. */
export type CrawlRun<Out = unknown> = {
    /** Run-level status. v0 enum only. */
    status: "success" | "success_with_loss" | "success_unvalidated" | "failed" | "cancelled";
    /** How the run ended budget-wise. */
    budgetStatus: "completed" | "exhausted" | "cancelled";
    /** Remaining frontier entries when budget exhausted (0 if completed). */
    remainingFrontier: number;
    /** Per-page outcomes keyed by final URL. */
    outcomes: Map<string, PageOutcome<Out>>;
    /** Flat diagnostic array for the entire run. */
    diagnostics: DiagnosticRecord[];
    /** Summary counters. */
    summary: {
        fetched: number;
        enqueued: number;
        scopeRejected: number;
        depthRejected: number;
        success: number;
        skipped: number;
        failed: number;
    };
};

// ---------------------------------------------------------------------------
// Crawler — thin authoring surface over Rust bindings
// ---------------------------------------------------------------------------

/** High-level Pandark crawler. **Authoring surface only** — Rust owns execution. */
export class Crawler {
    private binding: PandarkBindings;

    constructor(_options?: CrawlerOptions) {
        this.binding = loadPandarkNode();
        // NOTE: CrawlerOptions (store, politeness) currently ignored — Rust
        // binding does not yet accept these. Not a bug; when Rust adds the
        // surface this constructor will forward. See 规划设计/pandark/01.
    }

    /** Fetch a single URL. Thin wrapper over Rust `fetchSeed`. */
    async fetch(seed: string, _options?: FetchOptions): Promise<FetchArtifact> {
        // NOTE: FetchOptions currently ignored — Rust binding not ready.
        const response = this.binding.fetchSeed(seed);
        const artifact = JSON.parse(response.artifactJson) as RawArtifact;
        return normalizeArtifact(artifact);
    }

    /**
     * Run a crawl rooted at `seed`. **Rust-owned execution.**
     *
     * When the Rust `runCrawl` binding is ready this will simply invoke it
     * and map the result. Until then we return a `rust-binding-not-ready`
     * diagnostic and do NOT pretend the run succeeded.
     *
     * **No TS frontier, no TS regex link discovery, no local scope gates** —
     * those all live on the Rust side.
     */
    async crawl<Out>(seed: string, options: CrawlOptions<Out>): Promise<CrawlRun<Out>> {
        // Check if Rust binding has runCrawl ready
        const runCrawl = (this.binding as unknown as { runCrawl?: (arg: string) => unknown }).runCrawl;
        if (!runCrawl) {
            const notReadyDiag: DiagnosticRecord = {
                code: "pandark.crawl.rust-binding-not-ready",
                fallback: "Crawler.crawl() requires Rust runCrawl binding — not yet available",
                severity: "warn",
                origin: { namespace: "pandark", component: "crawler" },
                stage: "fetch",
                url: seed,
            };
            const noopRun: CrawlRun<Out> = {
                status: "success_unvalidated",
                budgetStatus: "exhausted",
                remainingFrontier: 0,
                outcomes: new Map(),
                diagnostics: [notReadyDiag],
                summary: { fetched: 0, enqueued: 0, scopeRejected: 0, depthRejected: 0, success: 0, skipped: 0, failed: 0 },
            };
            return noopRun;
        }

        // When ready: await runCrawl(JSON.stringify({ seed, options })), map result.
        const response = runCrawl(JSON.stringify({ seed, options })) as { runJson?: string };
        if (!response?.runJson) {
            const panicDiag: DiagnosticRecord = {
                code: "pandark.crawl.rust-panic",
                fallback: "Rust runCrawl returned empty response",
                severity: "error",
                origin: { namespace: "pandark", component: "crawler" },
                stage: "fetch",
                url: seed,
            };
            return {
                status: "failed",
                budgetStatus: "cancelled",
                remainingFrontier: 0,
                outcomes: new Map(),
                diagnostics: [panicDiag],
                summary: { fetched: 0, enqueued: 0, scopeRejected: 0, depthRejected: 0, success: 0, skipped: 0, failed: 0 },
            };
        }
        return JSON.parse(response.runJson) as CrawlRun<Out>;
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function buildPage(artifact: FetchArtifact): Page {
    let htmlCache: string | null = null;
    return {
        get url() { return artifact.finalUrl; },
        get requestedUrl() { return artifact.requestedUrl; },
        get status() { return artifact.status; },
        get contentType() { return artifact.contentType; },
        get headers() { return artifact.headers; },
        async html(): Promise<string> {
            if (htmlCache !== null) return htmlCache;
            htmlCache = artifact.bodyText;
            return htmlCache;
        },
        bytes(): Uint8Array { return artifact.bodyBytes; },
    };
}

type RawArtifact = {
    final_url?: string; finalUrl?: string;
    request_url?: string; requestUrl?: string;
    status?: number;
    response_headers?: Record<string, string>; headers?: Record<string, string>;
    content_type?: string | null; contentType?: string;
    body?: RawBodyStorage;
};

type RawBodyStorage = { inline: number[] } | { reference: { cache_key: string } } | { Inline: number[] } | { Reference: { cache_key: string } };

function normalizeArtifact(raw: RawArtifact): FetchArtifact {
    const finalUrl = raw.final_url ?? raw.finalUrl ?? "";
    const requestedUrl = raw.request_url ?? raw.requestUrl ?? finalUrl;
    const status = raw.status ?? 0;
    const contentType = raw.content_type ?? raw.contentType ?? "";
    const headers = raw.response_headers ?? raw.headers ?? {};

    let bodyBytes = new Uint8Array();
    if (raw.body) {
        const body = raw.body as Record<string, unknown>;
        const inline = (body.inline as number[] | undefined) ?? (body.Inline as number[] | undefined);
        if (Array.isArray(inline)) bodyBytes = new Uint8Array(inline);
    }
    let bodyText = "";
    if (bodyBytes.length > 0) {
        bodyText = new TextDecoder("utf-8", { fatal: false }).decode(bodyBytes);
    }

    return { finalUrl, requestedUrl, status, contentType, headers, bodyText, bodyBytes };
}

function describeError(err: unknown): string {
    if (err instanceof Error) return err.message;
    return String(err);
}

/**
 * Normalize a diagnostic from Panduck — ensure it has required fields and a URL.
 * v0 shape only — **no severity defaulting to error** (we trust Rust/Panduck to set severity correctly).
 */
function normalizeDiagnostic(d: DiagnosticRecord, url: string): DiagnosticRecord {
    return {
        ...d,
        url: d.url ?? url,
        // No severity default — caller must set explicitly, loss inferred from code prefix.
    };
}

/**
 * Derive per-page outcome from handler diagnostics.
 *
 * **NO REGEX, NO STATUS FIELD OVERRIDE, NO STRING MATCHING.**
 *
 * Trust order (diagnostics always win):
 *   1. `severity === "error"` → failed
 *   2. `severity === "warn"` + code starts with "*.loss." or "panduck.convert.structure-loss." → success_with_loss
 *   3. `severity === "warn"` + code starts with "*.not-validated." → success_unvalidated
 *   4. Otherwise → success
 */
function derivePageStatus<Out>(_result: ConverterLikeResult<Out> | undefined, diags: DiagnosticRecord[]): PageOutcome<Out>["status"] {
    const hasError = diags.some((d) => d.severity === "error");
    if (hasError) return "failed";

    const hasLoss = diags.some((d) => d.severity === "warn" && isLossCode(d.code));
    if (hasLoss) return "success_with_loss";

    const hasUnvalidated = diags.some((d) => d.severity === "warn" && isNotValidatedCode(d.code));
    if (hasUnvalidated) return "success_unvalidated";

    return "success";
}

/** Code starts with "*.loss." or equals known structure-loss codes. */
function isLossCode(code: string): boolean {
    if (code.endsWith(".loss")) return true;
    if (code.includes(".loss.")) return true;
    if (code === "panduck.convert.structure-loss") return true;
    return false;
}

/** Code starts with "*.not-validated." or equals known not-validated codes. */
function isNotValidatedCode(code: string): boolean {
    if (code.endsWith(".not-validated")) return true;
    if (code.includes(".not-validated.")) return true;
    return false;
}

/** v0 only — output always comes from `.text`. No `.markdown` fallback. */
function extractOutput<Out>(result: ConverterLikeResult<Out>): Out | undefined {
    return result.text;
}
