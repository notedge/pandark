/** Hard-coded render limits published by `@notedge/pandark-browser-worker`. */
export const BROWSER_LIMITS = {
    /** Lowest accepted per-page navigation budget. */
    minPageTimeoutMs: 1_000,
    /** Highest accepted per-page navigation budget. */
    maxPageTimeoutMs: 30_000,
    /** Default per-page navigation budget. */
    defaultPageTimeoutMs: 20_000,
    /** Default DOM settle budget after `readyState` reaches `complete`. */
    defaultSettleMs: 5_000,
    /** Highest accepted DOM settle budget. */
    maxSettleMs: 10_000,
    /** Consecutive animation frames without a DOM mutation that count as settled. */
    quietFrames: 2,
    /** Captures rendered at the same time inside one worker. */
    maxConcurrentCaptures: 1,
    /** Captures that may wait for the single render slot. */
    maxQueuedCaptures: 8,
    /** Largest serialized `document_html` accepted in a snapshot. */
    maxSnapshotBytes: 8 * 1024 * 1024,
    /** Largest response byte total counted for one page. */
    maxNetworkBytes: 32 * 1024 * 1024,
    /** Largest main-document redirect chain accepted for one page. */
    maxRedirects: 4,
    /** Largest DOM node depth accepted in a snapshot. Chromium crashes near 250 levels in this build. */
    maxDomDepth: 128,
    /** Viewport used for every capture. */
    viewport: { width: 1_280, height: 800 },
} as const;

/** Structured worker error codes returned in `x-pandark-code`. */
export const BROWSER_ERROR_STATUS = {
    "pandark.browser.request-invalid": 400,
    "pandark.browser.private-address-denied": 403,
    "pandark.browser.overloaded": 429,
    "pandark.browser.unavailable": 503,
    "pandark.browser.navigate-failed": 502,
    "pandark.browser.render-timeout": 504,
    "pandark.browser.redirect-limit": 508,
    "pandark.browser.dom-depth-limit": 422,
    "pandark.browser.snapshot-too-large": 413,
    "pandark.browser.page-budget": 413,
} as const;

/** Stable identifier of one structured worker error. */
export type BrowserErrorCode = keyof typeof BROWSER_ERROR_STATUS;

/** Error raised by the render front-end. Always carries a public code. */
export class BrowserError extends Error {
    readonly code: BrowserErrorCode;

    readonly status: number;

    constructor(code: BrowserErrorCode, detail: string) {
        super(`${code}: ${detail}`);
        this.name = "BrowserError";
        this.code = code;
        this.status = BROWSER_ERROR_STATUS[code];
    }
}

/** Normalize a requested page timeout into the published hard bounds. */
export function clampPageTimeout(value: number | undefined): number {
    const requested = value ?? BROWSER_LIMITS.defaultPageTimeoutMs;
    if (!Number.isFinite(requested)) return BROWSER_LIMITS.defaultPageTimeoutMs;
    return Math.min(
        BROWSER_LIMITS.maxPageTimeoutMs,
        Math.max(BROWSER_LIMITS.minPageTimeoutMs, Math.floor(requested)),
    );
}

/** Normalize a requested settle budget into the published hard bounds. */
export function clampSettle(value: number | undefined): number {
    const requested = value ?? BROWSER_LIMITS.defaultSettleMs;
    if (!Number.isFinite(requested)) return BROWSER_LIMITS.defaultSettleMs;
    return Math.min(BROWSER_LIMITS.maxSettleMs, Math.max(0, Math.floor(requested)));
}
