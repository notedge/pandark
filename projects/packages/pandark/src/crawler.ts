/**
 * Pandark Crawler — public authoring surface for fetch + crawl.
 *
 * Mirrors the Crawlee/Scrapy/Colly-inspired shape frozen in the design doc,
 * intentionally small: Crawler owns fetch/frontier/politeness/cache and
 * exposes a `handlePage(page)` hook where user code directly returns the
 * Panduck ConverterResult. Crawler collects all diagnostics — fetch
 * failures, handler exceptions, and Converter diagnostics — into a unified
 * run report.
 */

import { loadPandarkNode } from "./node/load.js";
import type { PandarkBindings } from "./types.js";

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/** Single fetched artifact from `crawler.fetch()`. */
export type FetchArtifact = {
    /** Final URL after redirects. */
    finalUrl: string;
    /** URL that was originally requested. */
    requestedUrl: string;
    /** HTTP status code (0 when non-HTTP source). */
    status: number;
    /** `content-type` header value. */
    contentType: string;
    /** Response headers (lower-case key → value). */
    headers: Record<string, string>;
    /** Response body decoded as UTF-8 text when available. */
    bodyText: string;
    /** Raw response bytes (always present). */
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
 *
 * Throwing inside handlePage is allowed — Crawler records it as a
 * per-page handler failure and continues with the next page.
 */
export type HandlePageFn<Out = unknown> = (
    page: Page,
) => Promise<ConverterLikeResult<Out>> | ConverterLikeResult<Out>;

/**
 * Structural duck-type over Panduck ConverterResult — Crawler must not
 * depend on Panduck at the type level (that would couple runtime edges).
 * Any object with `{ diagnostics?: DiagnosticRecord[], text?: unknown }`
 * satisfies the contract.
 */
export type ConverterLikeResult<Out = unknown> = {
    /** Panduck diagnostic envelope (or compatible shape). */
    diagnostics?: DiagnosticRecord[];
    /** Primary output value — typically Markdown text. */
    text?: Out;
    /** Compatibility: callers may also supply `status`. */
    status?: string;
    /** Compatibility: callers may also supply `markdown`. */
    markdown?: string;
};

/**
 * Structured diagnostic record shared across Pandark + Panduck.
 *
 * Mirrors `boost-diagnostic::Diagnostic` but kept as a plain TS interface so
 * the Crawler does not need to know about Rust binding internals.
 */
export type DiagnosticRecord = {
    /** Stable dotted code — e.g. `pandark.fetch.transport-failed`, `panduck.convert.required-missing`. */
    code: string;
    /** Fallback human-readable message. */
    fallback?: string;
    /** Message key for locale — e.g. `pandark.diagnostic.transportFailed`. */
    messageKey?: string;
    /** Typed args — stable keys, free-form values. */
    args?: Record<string, unknown>;
    /** Origin identity. */
    origin?: {
        namespace: string;
        component: string;
    };
    /** Severity. */
    severity?: "error" | "warn" | "info";
    /** When the diagnostic was produced. */
    stage?: "fetch" | "scope" | "structure" | "convert" | "rule" | "handler";
    /** URL the diagnostic belongs to. */
    url?: string;
    /** Affected source path / selector / asset reference. */
    target?: string;
};

/** Options for `new Crawler()`. */
export type CrawlerOptions = {
    store?: { path: string };
    politeness?: "aggressive" | "conservative";
};

/** Options for `crawler.fetch()`. */
export type FetchOptions = {
    /** Browser endpoint for headless fallback. */
    browserEndpoint?: string;
};

/** Options for `crawler.crawl()`. */
export type CrawlOptions<Out = unknown> = {
    scope?: CrawlScope;
    limits?: CrawlLimits;
    handlePage: HandlePageFn<Out>;
};

/** Per-page outcome recorded during a crawl. */
export type PageOutcome<Out = unknown> = {
    url: string;
    status: "success" | "success_with_loss" | "failed" | "skipped";
    output?: Out;
    diagnostics: DiagnosticRecord[];
};

/** Full crawl run report. */
export type CrawlRun<Out = unknown> = {
    /** Run-level status. */
    status: "success" | "partial" | "failed";
    /** Pages processed (success + skipped + failed). */
    pages: number;
    /** Per-page outcomes keyed by final URL. */
    outcomes: Map<string, PageOutcome<Out>>;
    /** Flat diagnostic array for the entire run. */
    diagnostics: DiagnosticRecord[];
    /** Engine-level summary. */
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
// Crawler
// ---------------------------------------------------------------------------

/** High-level Pandark crawler — entry point for fetch + crawl. */
export class Crawler {
    private binding: PandarkBindings;

    constructor(_options?: CrawlerOptions) {
        this.binding = loadPandarkNode();
    }

    /** Fetch a single URL and return the artifact. No frontier, no handlers. */
    async fetch(seed: string, _options?: FetchOptions): Promise<FetchArtifact> {
        const response = this.binding.fetchSeed(seed);
        const artifact = JSON.parse(response.artifactJson) as RawArtifact;
        return normalizeArtifact(artifact);
    }

    /**
     * Run a crawl rooted at `seed`. For every page that passes politeness and
     * scope gates, invoke `handlePage(page)` and collect its result + diagnostics.
     *
     * Crawler owns frontier scheduling, deduplication, host-scope gating,
     * and link discovery. It does NOT re-parse HTML — Panduck remains the
     * single source for full HTML lowering.
     */
    async crawl<Out>(seed: string, options: CrawlOptions<Out>): Promise<CrawlRun<Out>> {
        const { handlePage, scope = {}, limits = {} } = options;

        const allowedHosts = new Set((scope.allowedHosts ?? []).map((h) => h.toLowerCase()));
        const allowedDomains = new Set((scope.allowedDomains ?? []).map((d) => d.toLowerCase()));
        const maxDepth = limits.maxDepth ?? 2;
        const maxRequests = limits.maxRequests ?? 100;

        const seen = new Set<string>();
        const frontier: Array<{ url: string; depth: number }> = [];
        frontier.push({ url: seed, depth: 0 });

        const outcomes = new Map<string, PageOutcome<Out>>();
        const diagnostics: DiagnosticRecord[] = [];
        let fetched = 0;
        let enqueued = 0;
        let scopeRejected = 0;
        let depthRejected = 0;
        let success = 0;
        let skipped = 0;
        let failed = 0;

        while (frontier.length > 0 && fetched < maxRequests) {
            const next = frontier.shift()!;
            if (seen.has(next.url)) continue;
            seen.add(next.url);

            const host = hostOf(next.url);
            if (allowedHosts.size > 0 && !allowedHosts.has(host)) {
                scopeRejected++;
                const rec = {
                    code: "pandark.scope.rejected",
                    fallback: `page rejected by host-scope gate: ${next.url}`,
                    messageKey: "pandark.diagnostic.scopeRejected",
                    origin: { namespace: "pandark", component: "crawler" },
                    severity: "info" as const,
                    stage: "scope" as const,
                    url: next.url,
                };
                diagnostics.push(rec);
                outcomes.set(next.url, { url: next.url, status: "skipped", diagnostics: [rec] });
                continue;
            }
            if (allowedDomains.size > 0 && !domainMatches(next.url, allowedDomains)) {
                scopeRejected++;
                const rec = {
                    code: "pandark.scope.rejected",
                    fallback: `page rejected by domain-scope gate: ${next.url}`,
                    messageKey: "pandark.diagnostic.scopeRejected",
                    origin: { namespace: "pandark", component: "crawler" },
                    severity: "info" as const,
                    stage: "scope" as const,
                    url: next.url,
                };
                diagnostics.push(rec);
                outcomes.set(next.url, { url: next.url, status: "skipped", diagnostics: [rec] });
                continue;
            }

            // Fetch
            fetched++;
            let artifact: FetchArtifact;
            let fetchDiag: DiagnosticRecord[] = [];
            try {
                artifact = await this.fetch(next.url);
            } catch (err) {
                failed++;
                const rec = {
                    code: "pandark.fetch.transport-failed",
                    fallback: `fetch failed for ${next.url}: ${describeError(err)}`,
                    messageKey: "pandark.diagnostic.transportFailed",
                    args: { message: String(err) },
                    origin: { namespace: "pandark", component: "fetch" },
                    severity: "error" as const,
                    stage: "fetch" as const,
                    url: next.url,
                };
                fetchDiag.push(rec);
                diagnostics.push(rec);
                outcomes.set(next.url, { url: next.url, status: "failed", diagnostics: fetchDiag });
                continue;
            }

            // Link discovery for frontier — deliberately small, no HTML AST fidelity.
            const pageUrl = artifact.finalUrl;
            const links = extractNavigationLinks(artifact.bodyText, pageUrl);
            for (const link of links) {
                const linkHost = hostOf(link);
                if (allowedHosts.size > 0 && !allowedHosts.has(linkHost)) continue;
                if (allowedDomains.size > 0 && !domainMatches(link, allowedDomains)) continue;
                if (next.depth + 1 > maxDepth) {
                    depthRejected++;
                    continue;
                }
                if (seen.has(link)) continue;
                frontier.push({ url: link, depth: next.depth + 1 });
                enqueued++;
            }

            // Build Page and invoke handlePage.
            const page = buildPage(artifact);
            let handlerOutcome: ConverterLikeResult<Out> | undefined;
            let handlerException: unknown = undefined;
            try {
                handlerOutcome = await handlePage(page);
            } catch (err) {
                handlerException = err;
            }

            const pageDiagnostics: DiagnosticRecord[] = [...fetchDiag];

            if (handlerException !== undefined) {
                failed++;
                const rec = {
                    code: "pandark.handler.failed",
                    fallback: `handlePage threw for ${pageUrl}: ${describeError(handlerException)}`,
                    messageKey: "pandark.diagnostic.handlerFailed",
                    args: { message: String(handlerException) },
                    origin: { namespace: "pandark", component: "crawler" },
                    severity: "error" as const,
                    stage: "handler" as const,
                    url: pageUrl,
                };
                pageDiagnostics.push(rec);
                diagnostics.push(rec);
                outcomes.set(pageUrl, { url: pageUrl, status: "failed", diagnostics: pageDiagnostics });
                continue;
            }

            if (handlerOutcome && Array.isArray(handlerOutcome.diagnostics)) {
                for (const d of handlerOutcome.diagnostics) {
                    pageDiagnostics.push(normalizeDiagnostic(d, pageUrl));
                    diagnostics.push(normalizeDiagnostic(d, pageUrl));
                }
            }

            // Derive page status from the converter result + diagnostics.
            const pageStatus = derivePageStatus(handlerOutcome, pageDiagnostics);
            if (pageStatus === "failed") failed++;
            else if (pageStatus === "skipped") skipped++;
            else success++;

            outcomes.set(pageUrl, {
                url: pageUrl,
                status: pageStatus,
                output: handlerOutcome ? extractOutput(handlerOutcome) : undefined,
                diagnostics: pageDiagnostics,
            });
        }

        const runStatus: CrawlRun<Out>["status"] =
            success === 0 && failed > 0
                ? "failed"
                : failed > 0
                ? "partial"
                : "success";

        return {
            status: runStatus,
            pages: fetched,
            outcomes,
            diagnostics,
            summary: { fetched, enqueued, scopeRejected, depthRejected, success, skipped, failed },
        };
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function buildPage(artifact: FetchArtifact): Page {
    let htmlCache: string | null = null;
    return {
        get url() {
            return artifact.finalUrl;
        },
        get requestedUrl() {
            return artifact.requestedUrl;
        },
        get status() {
            return artifact.status;
        },
        get contentType() {
            return artifact.contentType;
        },
        get headers() {
            return artifact.headers;
        },
        async html(): Promise<string> {
            if (htmlCache !== null) return htmlCache;
            htmlCache = artifact.bodyText;
            return htmlCache;
        },
        bytes(): Uint8Array {
            return artifact.bodyBytes;
        },
    };
}

type RawArtifact = {
    final_url?: string;
    finalUrl?: string;
    request_url?: string;
    requestUrl?: string;
    status?: number;
    response_headers?: Record<string, string>;
    headers?: Record<string, string>;
    content_type?: string | null;
    contentType?: string;
    body?: RawBodyStorage;
};

type RawBodyStorage =
    | { inline: number[] }
    | { reference: { cache_key: string } }
    | { Inline: number[] }
    | { Reference: { cache_key: string } };

function normalizeArtifact(raw: RawArtifact): FetchArtifact {
    const finalUrl = raw.final_url ?? raw.finalUrl ?? "";
    const requestedUrl = raw.request_url ?? raw.requestUrl ?? finalUrl;
    const status = raw.status ?? 0;
    const contentType = raw.content_type ?? raw.contentType ?? "";
    const headers = raw.response_headers ?? raw.headers ?? {};

    let bodyBytes = new Uint8Array();
    if (raw.body) {
        const body = raw.body as Record<string, unknown>;
        const inline =
            (body.inline as number[] | undefined) ??
            (body.Inline as number[] | undefined);
        if (Array.isArray(inline)) {
            bodyBytes = new Uint8Array(inline);
        }
    }

    let bodyText = "";
    if (bodyBytes.length > 0) {
        bodyText = new TextDecoder("utf-8", { fatal: false }).decode(bodyBytes);
    }

    return { finalUrl, requestedUrl, status, contentType, headers, bodyText, bodyBytes };
}

function hostOf(url: string): string {
    try {
        return new URL(url).hostname.toLowerCase();
    } catch {
        return "";
    }
}

function domainMatches(url: string, allowed: Set<string>): boolean {
    const host = hostOf(url);
    for (const candidate of allowed) {
        if (host === candidate) return true;
        if (host.endsWith("." + candidate)) return true;
    }
    return false;
}

const HREF_RE = /<(?:a|area)\b[^>]*?\bhref\s*=\s*(?:"([^"]+)"|'([^']+)'|([^\s>]+))/gi;

function extractNavigationLinks(html: string, baseUrl: string): string[] {
    const result: string[] = [];
    const seen = new Set<string>();
    for (const m of html.matchAll(HREF_RE)) {
        const target = (m[1] ?? m[2] ?? m[3] ?? "").trim();
        if (!target || target.startsWith("#") || target.startsWith("javascript:")) continue;
        let absolute: string;
        try {
            absolute = new URL(target, baseUrl).toString();
        } catch {
            continue;
        }
        if (seen.has(absolute)) continue;
        seen.add(absolute);
        result.push(absolute);
    }
    return result;
}

function describeError(err: unknown): string {
    if (err instanceof Error) return err.message;
    return String(err);
}

/** Normalize a diagnostic from Panduck — ensure it has required fields and a URL. */
function normalizeDiagnostic(d: DiagnosticRecord, url: string): DiagnosticRecord {
    return {
        ...d,
        url: d.url ?? url,
        severity: d.severity ?? "error",
        stage: d.stage ?? "convert",
    };
}

/**
 * Derive per-page outcome from the handler result + diagnostics.
 *
 * Trust order (diagnostics always win):
 *   1. Error-severity diagnostics → failed, regardless of what `result.status` claims.
 *   2. `result.status === "failed"` / `"error"` → failed.
 *   3. Loss diagnostics OR `result.status === "success_with_loss"` → success_with_loss.
 *   4. Otherwise → success.
 *
 * We deliberately do NOT let a raw `"success"` status override error
 * diagnostics — that was the old bug where a caller could declare success
 * while leaving error records that consumers would then miss.
 */
function derivePageStatus<Out>(result: ConverterLikeResult<Out> | undefined, diags: DiagnosticRecord[]): PageOutcome<Out>["status"] {
    if (!result) return "failed";
    // Error diagnostics always win — no caller can override these.
    const hasError = diags.some((d) => d.severity === "error");
    if (hasError) return "failed";
    // Explicit failure status is respected (no contradiction).
    if (result.status === "failed" || result.status === "error") return "failed";
    // Known structural loss?
    const hasLoss = diags.some((d) => /loss|required|missing|rule\.|not-validated/i.test(d.code))
        || result.status === "success_with_loss";
    if (hasLoss) return "success_with_loss";
    return "success";
}

function extractOutput<Out>(result: ConverterLikeResult<Out>): Out | undefined {
    if (result.text !== undefined) return result.text;
    if (result.markdown !== undefined) return result.markdown as Out;
    return undefined;
}
