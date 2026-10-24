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
 * `Crawler.crawl()` has two execution paths:
 *
 *   - **Single-page** (no limits or trivial limits): Rust `fetchSeed` +
 *     user-supplied `handlePage` callback. Only the seed page is visited.
 *
 *   - **Rust run** (`limits.maxRequests > 1`, `limits.maxDepth > 0`, a
 *     `scope`, or a configured `politeness`): Rust `crawlFile` owns frontier
 *     scheduling, link discovery, scope gates and budget enforcement. TS
 *     receives a `CrawlReport` JSON and assembles a `CrawlRun` from it.
 *     **Note:** in this path `handlePage` is NOT invoked — Rust performs
 *     fetch + extract natively; TS sees summary counters, per-page URL-level
 *     outcomes, and the run-level diagnostic envelope. Per-page artifact
 *     delivery to `handlePage` is a future expansion once N-API surfaces
 *     artifacts alongside the report.
 */

import { loadPandarkNode } from "./node/load.js";
import type { PandarkBindings, MarkdownResult } from "./types.js";
import { createPageView } from "./page-view.js";
import { runExtraction } from "./extraction-result.js";
import type { ExtractionProfile } from "./extraction-profile.js";
import type { ExtractionResult } from "./extraction-result.js";

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

/** Options for `new Crawler()`. */
export type CrawlerOptions = {
    /**
     * Scheduling politeness preset forwarded to the Rust crawl runtime.
     * `aggressive` maps to the Rust developer profile.
     */
    politeness?: "conservative" | "balanced" | "aggressive";
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
    private politeness: CrawlerOptions["politeness"];

    constructor(options?: CrawlerOptions) {
        this.binding = loadPandarkNode();
        this.politeness = options?.politeness;
    }

    /**
     * Fetch a single URL. Thin wrapper over Rust `fetchSeed`.
     *
     * `binding.fetchSeed` is an **N-API synchronous** function — this method
     * body performs zero awaits. The `async` signature is kept deliberately so
     * the public surface stays uniform when future provider backends (browser,
     * real HTTP client) introduce genuinely async bindings. Callers may safely
     * `await` the return value; the Promise resolves immediately after the
     * synchronous Rust call returns.
     */
    async fetch(seed: string): Promise<FetchArtifact> {
        const response = this.binding.fetchSeed(seed);
        const artifact = JSON.parse(response.artifactJson) as RawArtifact;
        return normalizeArtifact(artifact);
    }

    /**
     * Run a crawl rooted at `seed`.
     *
     * The Rust crawl runtime is used when the caller sets multi-page limits,
     * a `scope`, or a crawler-level `politeness`. Rust `crawlFile` owns the
     * whole run; TS assembles the `CrawlRun` from the returned `CrawlReport`.
     * `handlePage` is **not** invoked on that path (Rust extracts natively).
     *
     * Single-page without scope or politeness is the default and invokes
     * `handlePage` as before.
     */
    async crawl<Out>(seed: string, options: CrawlOptions<Out>): Promise<CrawlRun<Out>> {
        const limits = options.limits;
        const multiPage = (limits?.maxRequests ?? 0) > 1 || (limits?.maxDepth ?? 0) > 0;
        const usesRustRun = multiPage || options.scope !== undefined || this.politeness !== undefined;

        if (usesRustRun) {
            // Scope and politeness only take effect inside the Rust crawl run;
            // route through `crawlFile` so they are never silently ignored.
            return this.runCrawlFile(seed, multiPage ? limits : { maxDepth: 0, maxRequests: 1 }, options);
        }

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

    /**
     * 拉取一个 URL，用 ExtractionProfile 抽取受控结果。
     *
     * 等价于 fetch(url) → createPageView(body) → runExtraction(view, profile)，
     * 但把所有 fetch/parse/extract 的诊断合并进返回的 ExtractionResult.diagnostics。
     *
     * 这是作者友好的入口 — 替代之前用户不得不手写 CSS selector 链的做法。
     *
     * 内部同样走 N-API 同步 binding.fetchSeed，方法体零 await；保留 async 签名
     * 是为了与未来真正的异步 provider（浏览器 / 真实 HTTP 客户端）接口统一。
     */
    async extractWithProfile(
        url: string,
        profile: ExtractionProfile,
    ): Promise<ExtractionResult> {
        let artifact: FetchArtifact;
        try {
            const response = this.binding.fetchSeed(url);
            const raw = JSON.parse(response.artifactJson) as RawArtifact;
            artifact = normalizeArtifact(raw);
        }
        catch (err) {
            // 拉取失败 — 返回一个只有 scopeStatus = no-match 的结果，
            // 里面灌满 fetch 相关诊断
            const fetchDiag: DiagnosticRecord = {
                code: "pandark.extraction.fetch-failed",
                fallback: `Failed to fetch page for extraction: ${describeError(err)}`,
                severity: "error",
                origin: { namespace: "pandark", component: "crawler" },
                stage: "fetch",
                url,
            };
            return {
                scopeStatus: "no-match",
                fields: {},
                assets: [],
                links: [],
                diagnostics: [fetchDiag],
            };
        }

        const view = createPageView(artifact.bodyText, artifact.finalUrl, {
            status: artifact.status,
            contentType: artifact.contentType,
        });
        return runExtraction(view, profile);
    }

    /**
     * One-shot convenience: fetch → Notedown IR → Markdown.
     *
     * This does NOT go through Panduck. The internal chain is:
     *
     *   fetchSeed → HTML bytes
     *   → notedown_formats::import::html
     *   → DocumentGraph (notedown-ir)
     *   → notedown_formats::export::markdown
     *   → Markdown string
     *
     * Pass through a page `profile` to pre-filter scope/fields on the TS
     * side before handing off to Rust. Without a profile the Rust side
     * performs full-page HTML → Markdown.
     *
     * 底层 `binding.fetchSeed` 与 `binding.extractToMarkdown` 均为 N-API
     * 同步调用。本方法内部 `await this.fetch()` 仅为保持调用链上的 Promise
     * 形状一致 — 待未来 async provider 接入后这些 await 才会真正等待。
     */
    async toMarkdown(
        url: string,
        options?: { profile?: ExtractionProfile; sourceUrl?: string },
    ): Promise<MarkdownResult> {
        const artifact = await this.fetch(url);
        const sourceUrl = options?.sourceUrl ?? artifact.finalUrl;

        if (options?.profile) {
            // Profile path: TS-side scope filter first, then hand filtered HTML to Rust.
            const view = createPageView(artifact.bodyText, artifact.finalUrl, {
                status: artifact.status,
                contentType: artifact.contentType,
            });
            const extracted = runExtraction(view, options.profile);
            // runExtraction produces bodyHtml (the scoped candidate) — convert that.
            const scopedBytes = new TextEncoder().encode(extracted.bodyHtml ?? artifact.bodyText);
            return this.binding.extractToMarkdown(scopedBytes, sourceUrl);
        }

        // No profile: full HTML → Markdown in one Rust call.
        return this.binding.extractToMarkdown(artifact.bodyBytes, sourceUrl);
    }

    /**
     * Multi-page path — delegate entirely to Rust `crawlFile`.
     *
     * Rust owns fetch, extract, frontier scheduling, scope gates, and
     * budget enforcement. TS assembles a `CrawlRun` from the returned
     * `CrawlReport` JSON. Per-page artifact delivery is not yet wired, so
     * `handlePage` is not invoked and outcome `output` is always undefined.
     */
    private runCrawlFile<Out>(seed: string, limits: CrawlLimits | undefined, options: CrawlOptions<Out>): CrawlRun<Out> {
        try {
            const response = this.binding.crawlFile(
                seed,
                limits?.maxDepth,
                limits?.maxRequests,
                undefined,
                undefined,
                undefined,
                undefined,
                undefined,
                options.scope?.allowedHosts ? [...options.scope.allowedHosts] : undefined,
                options.scope?.allowedDomains ? [...options.scope.allowedDomains] : undefined,
                this.politeness,
            );
            return assembleRunFromCrawlFile<Out>(response);
        }
        catch (err) {
            const diag: DiagnosticRecord = {
                code: "pandark.crawl.crawlfile-failed",
                fallback: `Rust crawlFile failed: ${describeError(err)}`,
                severity: "error",
                origin: { namespace: "pandark", component: "crawler" },
                stage: "fetch",
                url: seed,
            };
            const outcome: PageOutcome<Out> = {
                url: seed,
                status: "failed",
                diagnostics: [diag],
            };
            return assembleRun(seed, outcome, [diag]);
        }
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

// ---------------------------------------------------------------------------
// Multi-page (crawlFile) path — Rust CrawlReport → TS CrawlRun
// ---------------------------------------------------------------------------

/** Shape of Rust `boost_diagnostic::Diagnostic` as serialized inside the report envelope. */
type RawRustDiagnostic = {
    code?: string;
    severity?: string;
    origin?: { namespace?: string; component?: string; stage?: string };
    message?: {
        key?: string;
        args?: Array<[string, unknown]>;
        fallback?: string;
    };
};

/** Shape of Rust `DiagnosticEnvelope`. */
type RawDiagnosticEnvelope = { diagnostics: RawRustDiagnostic[] };

/** Rust-side admission decision — externally tagged enum. */
type RawAdmissionDecision = "admitted" | { rejected: { reason: string } };

/** Union of Rust `CrawlEvent` variants as serialized (tagged by `kind`). */
type RawCrawlEvent =
    | { kind: "discovered"; url: string; depth: number }
    | { kind: "admitted"; url: string; decision: RawAdmissionDecision }
    | { kind: "fetched"; url: string; status: number; cache_hit: boolean }
    | { kind: "browser_fallback"; url: string; fetch_error: string; browser_engine: string }
    | { kind: "extracted"; url: string; status: string; extractor: string }
    | { kind: "committed"; page_identity: string }
    | { kind: "skipped"; url: string; reason: string }
    | { kind: "failed"; url: string; reason: string }
    | { kind: "paused_for_operator"; url: string; challenge_state: unknown };

/** Shape of Rust `CrawlReport` JSON (schema_version = "pandark.report/v0"). */
type RawCrawlReport = {
    schema_version: string;
    seed: string;
    extracted: number;
    failed: number;
    skipped: number;
    paused: number;
    committed: number;
    budget_status: "completed" | "exhausted" | "cancelled";
    remaining_frontier: number;
    events: { events: RawCrawlEvent[] };
    diagnostics: RawDiagnosticEnvelope;
};

/**
 * Assemble a `CrawlRun` from the Rust `crawlFile` response.
 *
 * Rust owns all fetch + extract work; TS just reshapes the report. Per-page
 * `output` is always undefined because no `handlePage` callback runs on
 * this path — Rust extracts natively.
 */
function assembleRunFromCrawlFile<Out>(response: {
    exitCode: number;
    reportJson: string;
    committedPages: string[];
    checkpointJson?: string;
}): CrawlRun<Out> {
    const report: RawCrawlReport = JSON.parse(response.reportJson);
    const events = report.events.events;

    // --- outcomes ---
    const outcomes = new Map<string, PageOutcome<Out>>();

    // Committed pages → success
    for (const url of response.committedPages) {
        outcomes.set(url, { url, status: "success", diagnostics: [] });
    }

    // Events tell us about skipped / failed pages that never committed.
    for (const event of events) {
        if (event.kind === "failed") {
            if (!outcomes.has(event.url)) {
                outcomes.set(event.url, {
                    url: event.url,
                    status: "failed",
                    diagnostics: [{
                        code: "pandark.crawl.page-failed",
                        fallback: event.reason,
                        severity: "error",
                        origin: { namespace: "pandark", component: "crawler" },
                        stage: "fetch",
                        url: event.url,
                    }],
                });
            }
        }
        else if (event.kind === "skipped") {
            if (!outcomes.has(event.url)) {
                outcomes.set(event.url, {
                    url: event.url,
                    status: "skipped",
                    diagnostics: [],
                });
            }
        }
        else if (event.kind === "paused_for_operator") {
            if (!outcomes.has(event.url)) {
                outcomes.set(event.url, {
                    url: event.url,
                    status: "skipped",
                    diagnostics: [],
                });
            }
        }
    }

    // --- run-level diagnostics ---
    const runDiags: DiagnosticRecord[] = (report.diagnostics?.diagnostics ?? []).map(normalizeRustDiagnostic);

    // --- summary counters derived from events ---
    const fetched = events.filter((e) => e.kind === "fetched").length;
    const enqueued = events.filter((e) => e.kind === "discovered").length;

    const admissionRejections = events.filter(
        (e): e is Extract<RawCrawlEvent, { kind: "admitted" }> =>
            e.kind === "admitted" && typeof e.decision === "object",
    );
    const rejectionReasons = admissionRejections.map((e) => rejectionReason(e.decision));
    const depthRejected = rejectionReasons.filter((reason) => reason === "pandark.reject.depth-exceeded").length;
    const scopeRejected = rejectionReasons.filter((reason) => reason === "pandark.reject.out-of-scope").length;

    // Scope rejections are part of the per-page ledger, not just a counter.
    for (const event of admissionRejections) {
        if (rejectionReason(event.decision) !== "pandark.reject.out-of-scope" || outcomes.has(event.url)) continue;
        outcomes.set(event.url, {
            url: event.url,
            status: "skipped",
            diagnostics: [{
                code: "pandark.scope.rejected",
                fallback: `URL is outside the crawl scope: ${event.url}`,
                severity: "info",
                origin: { namespace: "pandark", component: "crawler" },
                stage: "scope",
                url: event.url,
            }],
        });
    }

    // --- run + budget status ---
    const status = deriveMultiRunStatus(report, outcomes);

    return {
        status,
        budgetStatus: report.budget_status,
        remainingFrontier: report.remaining_frontier,
        outcomes,
        diagnostics: runDiags,
        summary: {
            fetched,
            enqueued,
            scopeRejected,
            depthRejected,
            success: report.extracted,
            skipped: report.skipped,
            failed: report.failed,
        },
    };
}

/** Extract the rejection reason from the externally tagged Rust enum. */
function rejectionReason(decision: RawAdmissionDecision): string | undefined {
    if (decision === "admitted") return undefined;
    return decision.rejected?.reason;
}

/**
 * Normalize a Rust `boost_diagnostic::Diagnostic` into the TS
 * `DiagnosticRecord` shape that downstream consumers expect.
 */
function normalizeRustDiagnostic(raw: RawRustDiagnostic): DiagnosticRecord {
    return {
        code: raw.code ?? "pandark.unknown",
        fallback: raw.message?.fallback,
        messageKey: raw.message?.key,
        args: raw.message?.args ? Object.fromEntries(raw.message.args) : undefined,
        origin: raw.origin?.namespace
            ? { namespace: raw.origin.namespace, component: raw.origin.component ?? "" }
            : undefined,
        severity: normalizeRustSeverity(raw.severity),
        stage: raw.origin?.stage as DiagnosticRecord["stage"],
    };
}

function normalizeRustSeverity(value: string | undefined): DiagnosticRecord["severity"] {
    switch (value) {
        case "bug":
        case "error":
            return "error";
        case "warning":
            return "warn";
        case "info":
        case "hint":
            return "info";
        default:
            return "error";
    }
}

/**
 * Derive run-level status for a Rust-backed crawl run.
 *
 * Rules:
 *   - any failed page or run-level error → "failed"
 *   - paused run → "cancelled"
 *   - budget exhausted without errors → "success_unvalidated" (never "success")
 *   - a page with loss → "success_with_loss"
 *   - otherwise → "success"
 */
function deriveMultiRunStatus<Out>(
    report: RawCrawlReport,
    outcomes: Map<string, PageOutcome<Out>>,
): CrawlRun<Out>["status"] {
    if (report.failed > 0 || (report.diagnostics?.diagnostics ?? []).some((d) => d.severity === "error")) {
        return "failed";
    }
    if (report.budget_status === "cancelled") return "cancelled";
    if (report.budget_status === "exhausted") return "success_unvalidated";
    if ([...outcomes.values()].some((outcome) => outcome.status === "success_with_loss")) return "success_with_loss";
    return "success";
}
