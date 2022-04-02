/**
 * Pandark Engine — public authoring surface for crawl + page handling.
 *
 * Reference shape after Crawlee / Scrapy / Colly, but intentionally much smaller:
 * Engine owns fetch/frontier/politeness/cache and exposes a `handlePage(page, context)`
 * hook where user code decides what to do with each page. Output goes into the
 * result sink, not directly to disk — downstream Panduck or user code writes files.
 */

import { loadPandarkNode } from "./node/load.js";
import type { PandarkBindings } from "./types.js";

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/** Result of `engine.fetch()` — one fetched artifact. */
export type FetchArtifact = {
    /** Final URL after redirects. */
    finalUrl: string;
    /** URL that was originally requested. */
    requestedUrl: string;
    /** HTTP status code. */
    status: number;
    /** `content-type` header value. */
    contentType: string;
    /** Response headers as lower-case key → value. */
    headers: Record<string, string>;
    /** Raw response body as UTF-8 text when the transport could decode it. */
    bodyText: string;
    /** Response body as raw bytes (always present even when bodyText is decoded). */
    bodyBytes: Uint8Array;
};

/** Page delivered to `handlePage`. */
export type Page = {
    /** Final URL after redirects. */
    readonly url: string;
    /** Original seed or frontier URL that triggered the fetch. */
    readonly requestedUrl: string;
    /** HTTP status code (0 when the page came from a non-HTTP source). */
    readonly status: number;
    /** Response content-type, e.g. `text/html; charset=utf-8`. */
    readonly contentType: string;
    /** Response headers. */
    readonly headers: Record<string, string>;
    /** Lazily decoded HTML/body string. Returns the same string on repeated calls. */
    html(): Promise<string>;
    /** Returns the raw HTML/body bytes without re-decoding. */
    bytes(): Uint8Array;
};

/** Navigation link discovered from a page during crawl. */
export type LinkCandidate = {
    /** Absolute URL of the link target. */
    url: string;
    /** How this link was discovered — href, src, form action, etc. */
    role: "navigation" | "asset" | "api";
    /** Anchor text or aria-label when present. */
    text?: string;
};

/** Page handler outcome passed back to the Engine. */
export type PageHandlerResult =
    | { kind: "output"; key?: string; value: unknown }
    | { kind: "skip"; reason?: string }
    | { kind: "error"; error: unknown };

/** Context object passed to `handlePage`. */
export type PageContext = {
    /** Enqueue navigation links that pass Pandark's policy gate. */
    enqueueLinks(): Promise<LinkCandidate[]>;
    /** Skip this page with an optional reason. */
    skip(reason?: string): PageHandlerResult;
    /** Emit a result into the crawl output sink. */
    output(entry: { key?: string; value: unknown }): PageHandlerResult;
};

/** Per-instance Engine options. */
export type EngineOptions = {
    /** Path to Pandark Yydb store directory. When omitted, crawl uses an in-memory frontier. */
    store?: { path: string };
    /** Default politeness profile for the instance. */
    politeness?: "aggressive" | "conservative";
};

/** Per-crawl options. */
export type CrawlOptions = {
    /** Host scope. Pages outside these hosts are not enqueued. */
    scope?: {
        allowedHosts?: readonly string[];
        allowedDomains?: readonly string[];
    };
    /** Hard budget limits for this crawl. */
    limits?: {
        maxDepth?: number;
        maxRequests?: number;
    };
    /** The user's page handler. Called once per crawled page. */
    handlePage: (page: Page, context: PageContext) => Promise<PageHandlerResult>;
};

/** Result of a full crawl run. */
export type CrawlRun = {
    /** Total pages processed (success + skipped + error). */
    pages: number;
    /** Handler outputs collected by key (or by index when key omitted). */
    outputs: Map<string, unknown>;
    /** Pages skipped by scope, depth, budget, or explicit handler return. */
    skipped: Array<{ url: string; reason: string }>;
    /** Handler errors collected during the run. */
    errors: Array<{ url: string; error: unknown }>;
    /** Engine-level summary (request counts, politeness events). */
    summary: {
        fetched: number;
        enqueued: number;
        scopeRejected: number;
        depthRejected: number;
    };
};

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/** High-level Pandark engine — entry point for crawl + page handling. */
export class Engine {
    private binding: PandarkBindings;
    private storePath?: string;

    constructor(options?: EngineOptions) {
        this.binding = loadPandarkNode();
        this.storePath = options?.store?.path;
    }

    /** Fetch a single URL and return the artifact. No crawl, no frontier. */
    async fetch(seed: string): Promise<FetchArtifact> {
        const response = this.binding.fetchSeed(seed);
        const artifact = JSON.parse(response.artifactJson) as RawArtifact;
        return normalizeArtifact(artifact);
    }

    /**
     * Run a crawl rooted at `seed`. For every page that passes the politeness
     * and host-scope gates, invoke `handlePage` with the page and a context.
     *
     * The Engine owns scheduling, caching, frontier, and link discovery. The
     * handler owns *what* to do with each page — convert it, emit a record,
     * skip it, or enqueue more links.
     */
    async crawl(seed: string, options: CrawlOptions): Promise<CrawlRun> {
        const { handlePage, scope = {}, limits = {} } = options;

        const allowedHosts = new Set((scope.allowedHosts ?? []).map((h) => h.toLowerCase()));
        const allowedDomains = new Set((scope.allowedDomains ?? []).map((d) => d.toLowerCase()));
        const maxDepth = limits.maxDepth ?? 2;
        const maxRequests = limits.maxRequests ?? 100;

        // Engine-owned frontier and deduplication. We run fetch ourselves so
        // that handlePage can be called from TS (not inside Rust). This is a
        // deliberate design choice — the Rust side owns fetch semantics and
        // politeness, the TS side owns the user hook composition.
        const seen = new Set<string>();
        const frontier: Array<{ url: string; depth: number; from?: string }> = [];
        frontier.push({ url: seed, depth: 0 });

        const outputs = new Map<string, unknown>();
        const skipped: CrawlRun["skipped"] = [];
        const errors: CrawlRun["errors"] = [];
        let nextOutputIndex = 0;
        let fetched = 0;
        let enqueued = 0;
        let scopeRejected = 0;
        let depthRejected = 0;

        while (frontier.length > 0 && fetched < maxRequests) {
            const next = frontier.shift()!;
            if (seen.has(next.url)) continue;
            seen.add(next.url);

            // Host-scope gate
            const host = hostOf(next.url);
            if (allowedHosts.size > 0 && !allowedHosts.has(host)) {
                scopeRejected++;
                skipped.push({ url: next.url, reason: "scope" });
                continue;
            }
            if (allowedDomains.size > 0 && !domainMatches(next.url, allowedDomains)) {
                scopeRejected++;
                skipped.push({ url: next.url, reason: "scope" });
                continue;
            }

            // Fetch
            fetched++;
            let artifact: FetchArtifact;
            try {
                artifact = await this.fetch(next.url);
            } catch (err) {
                errors.push({ url: next.url, error: err });
                continue;
            }

            // Build the Page and a context that lets the handler enqueue/skip/output.
            const page = buildPage(artifact);
            const discoveredLinks: LinkCandidate[] = extractLinksFromHtml(artifact.bodyText, artifact.finalUrl);

            const context: PageContext = {
                async enqueueLinks() {
                    const admitted: LinkCandidate[] = [];
                    for (const link of discoveredLinks) {
                        if (link.role !== "navigation") continue;
                        const linkHost = hostOf(link.url);
                        if (allowedHosts.size > 0 && !allowedHosts.has(linkHost)) continue;
                        if (allowedDomains.size > 0 && !domainMatches(link.url, allowedDomains)) continue;
                        if (next.depth + 1 > maxDepth) {
                            depthRejected++;
                            continue;
                        }
                        if (seen.has(link.url)) continue;
                        frontier.push({ url: link.url, depth: next.depth + 1, from: artifact.finalUrl });
                        enqueued++;
                        admitted.push(link);
                    }
                    return admitted;
                },
                skip(reason) {
                    skipped.push({ url: artifact.finalUrl, reason: reason ?? "handler" });
                    return { kind: "skip" as const, reason };
                },
                output(entry) {
                    const key = entry.key ?? String(nextOutputIndex++);
                    outputs.set(key, entry.value);
                    return { kind: "output" as const, key, value: entry.value };
                },
            };

            // First, let the handler enqueue whatever it wants.
            // The handler returns its final outcome; intermediate enqueueLinks/output/skip calls
            // happen through the context.
            let handlerOutcome: PageHandlerResult;
            try {
                handlerOutcome = await handlePage(page, context);
            } catch (err) {
                errors.push({ url: artifact.finalUrl, error: err });
                continue;
            }

            switch (handlerOutcome.kind) {
                case "output": {
                    const key = handlerOutcome.key ?? String(nextOutputIndex++);
                    outputs.set(key, handlerOutcome.value);
                    break;
                }
                case "skip": {
                    skipped.push({ url: artifact.finalUrl, reason: handlerOutcome.reason ?? "handler" });
                    break;
                }
                case "error": {
                    errors.push({ url: artifact.finalUrl, error: handlerOutcome.error });
                    break;
                }
            }
        }

        return {
            pages: fetched,
            outputs,
            skipped,
            errors,
            summary: { fetched, enqueued, scopeRejected, depthRejected },
        };
    }
}

// ---------------------------------------------------------------------------
// Helpers (not exported — Engine internals)
// ---------------------------------------------------------------------------

/** Raw JSON shape returned by Rust `fetch_seed` (serde snake_case, BodyStorage tagged enum). */
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
    body_text?: string;
};

/** Tagged BodyStorage from serde(snake_case). Only Inline is consumed here;
 *  Reference bodies mean the cache backend is out-of-line and not served to
 *  TS yet — normalizeArtifact returns an empty bodyBytes in that case. */
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

    // Rust BodyStorage tagged enum: { inline: [...] } or { reference: { cache_key: ... } }
    if (raw.body) {
        const body = raw.body as Record<string, unknown>;
        const inline =
            (body.inline as number[] | undefined) ??
            (body.Inline as number[] | undefined);
        if (Array.isArray(inline)) {
            bodyBytes = new Uint8Array(inline);
        }
        // Reference variant: bytes live in Pandark cache, not serialized here.
        // Engine returns an empty body so handlePage knows the body was
        // offloaded; future cache-reopen work will serve it back.
    }

    let bodyText = raw.body_text ?? "";
    if (bodyText === "" && bodyBytes.length > 0) {
        bodyText = new TextDecoder("utf-8", { fatal: false }).decode(bodyBytes);
    }

    return { finalUrl, requestedUrl, status, contentType, headers, bodyText, bodyBytes };
}

function buildPage(artifact: FetchArtifact): Page {
    // Cache decoded html on first access.
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

// ---------------------------------------------------------------------------
// Link extraction — a deliberately small, well-scoped parser. The Engine owns
// discovery semantics (navigation vs asset vs api), not HTML AST fidelity.
// Notedown / Panduck remain the single source for full HTML lowering.
// ---------------------------------------------------------------------------

const HREF_RE = /<(?:a|area)\b[^>]*?\bhref\s*=\s*(?:"([^"]+)"|'([^']+)'|([^\s>]+))/gi;
const SRC_RE = /<(?:img|script|iframe|video|audio|source)\b[^>]*?\bsrc\s*=\s*(?:"([^"]+)"|'([^']+)'|([^\s>]+))/gi;
const ACTION_RE = /<form\b[^>]*?\baction\s*=\s*(?:"([^"]+)"|'([^']+)'|([^\s>]+))/gi;

function extractLinksFromHtml(html: string, baseUrl: string): LinkCandidate[] {
    const result: LinkCandidate[] = [];
    const pushed = new Set<string>();

    const push = (target: string, role: LinkCandidate["role"]) => {
        let absolute: string;
        try {
            absolute = new URL(target, baseUrl).toString();
        } catch {
            return;
        }
        if (pushed.has(absolute)) return;
        pushed.add(absolute);
        result.push({ url: absolute, role });
    };

    for (const m of html.matchAll(HREF_RE)) {
        const target = (m[1] ?? m[2] ?? m[3] ?? "").trim();
        if (!target || target.startsWith("#") || target.startsWith("javascript:")) continue;
        push(target, "navigation");
    }
    for (const m of html.matchAll(SRC_RE)) {
        const target = (m[1] ?? m[2] ?? m[3] ?? "").trim();
        if (!target) continue;
        push(target, "asset");
    }
    for (const m of html.matchAll(ACTION_RE)) {
        const target = (m[1] ?? m[2] ?? m[3] ?? "").trim();
        if (!target) continue;
        push(target, "navigation");
    }

    return result;
}
