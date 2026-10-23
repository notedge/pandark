/**
 * Pandark Crawler — public authoring surface for fetch + crawl.
 *
 * Crawler is an **authoring surface** only. Frontier scheduling, scope
 * gating, budget management, link discovery, and politeness all live on the
 * **Rust side** (via `loadPandarkNode()` bindings). The TS Crawler does NOT
 * re-implement any of those — it validates inputs, invokes Rust bindings,
 * and maps the result back.
 *
 * `Crawler.fetch()` wraps Rust `fetchSeed` for single-page artifact retrieval.
 * `Crawler.crawl()` uses `fetchSeed` + user-supplied `handlePage` callback
 * for single-page fetch + convert. Multi-page scheduling through Rust
 * `crawlFile` is the future expansion path.
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
     * Run a crawl rooted at `seed`. **Rust-owned fetch, TS-owned callback.**
     *
     * This implementation fetches the seed page through Rust `fetchSeed`,
     * delivers it to `handlePage`, and returns a real `CrawlRun`. Frontier
     * scheduling, link discovery, scope gates, and multi-page budget all
     * remain on the Rust side via `crawlFile` — when those are ready this
     * method will delegate to it. For now we do single-page fetch + convert.
     */
    async crawl<Out>(seed: string, options: CrawlOptions<Out>): Promise<CrawlRun<Out>> {
        // Extract scope/limits — currently passed through; Rust-side enforcement
        // is the source of truth, but single-page fetch cannot respect them yet.
        const _scope = options.scope;
        const _limits = options.limits;

        let artifact: FetchArtifact;
        try {
            const response = this.binding.fetchSeed(seed);
            const raw = JSON.parse(response.artifactJson) as RawArtifact;
            artifact = normalizeArtifact(raw);
        }
        catch (err) {
            const fetchDiag: DiagnosticRecord = {
                code: "pandark.crawl.fetch-failed",
                fallback: `Failed to fetch seed: ${describeError(err)}`,
                severity: "error",
                origin: { namespace: "pandark", component: "crawler" },
                stage: "fetch",
                url: seed,
            };
            const outcome: PageOutcome<Out> = {
                url: seed,
                status: "failed",
                diagnostics: [fetchDiag],
            };
            return assembleRun(seed, outcome, [fetchDiag]);
        }

        const page = buildPage(artifact);

        let handlerResult: ConverterLikeResult<Out>;
        try {
            handlerResult = await options.handlePage(page);
        }
        catch (err) {
            const handlerDiag: DiagnosticRecord = {
                code: "pandark.crawl.handler-threw",
                fallback: `handlePage threw: ${describeError(err)}`,
                severity: "error",
                origin: { namespace: "pandark", component: "crawler" },
                stage: "handler",
                url: artifact.finalUrl,
            };
            const outcome: PageOutcome<Out> = {
                url: artifact.finalUrl,
                status: "failed",
                diagnostics: [handlerDiag],
            };
            return assembleRun(artifact.finalUrl, outcome, [handlerDiag]);
        }

        const diags = (handlerResult.diagnostics ?? []).map((d) => normalizeDiagnostic(d, artifact.finalUrl));
        const pageStatus = derivePageStatus(handlerResult, diags);
        const outcome: PageOutcome<Out> = {
            url: artifact.finalUrl,
            status: pageStatus,
            output: extractOutput(handlerResult),
            diagnostics: diags,
        };

        return assembleRun(artifact.finalUrl, outcome, diags);
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

/**
 * Assemble a CrawlRun from a single-page outcome.
 *
 * Run-level status mirrors page status:
 *   - page "failed" → run "failed"
 *   - page "success_with_loss" → run "success_with_loss"
 *   - page "success_unvalidated" → run "success_unvalidated"
 *   - page "success" or "skipped" → run "success"
 *
 * Summary counters reflect single-page fetch + convert.
 */
function assembleRun<Out>(
    url: string,
    outcome: PageOutcome<Out>,
    runDiags: DiagnosticRecord[],
): CrawlRun<Out> {
    const outcomes = new Map<string, PageOutcome<Out>>();
    outcomes.set(outcome.url, outcome);

    const runStatus: CrawlRun<Out>["status"] = outcome.status === "failed"
        ? "failed"
        : outcome.status === "skipped"
            ? "success"
            : outcome.status;

    const summary = {
        fetched: 1,
        enqueued: 1,
        scopeRejected: 0,
        depthRejected: 0,
        success: outcome.status === "success" || outcome.status === "success_with_loss" || outcome.status === "success_unvalidated" ? 1 : 0,
        skipped: outcome.status === "skipped" ? 1 : 0,
        failed: outcome.status === "failed" ? 1 : 0,
    };

    return {
        status: runStatus,
        budgetStatus: "completed",
        remainingFrontier: 0,
        outcomes,
        diagnostics: runDiags,
        summary,
    };
}
