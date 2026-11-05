import { lookup } from "node:dns/promises";
import { chromium, type Browser, type BrowserContext, type Page } from "playwright";

import { BROWSER_LIMITS, BrowserError, clampPageTimeout, clampSettle } from "./limits.js";

/** Challenge states accepted by the crawler snapshot contract. */
export type ChallengeState =
    | "normal"
    | "login_required"
    | "challenge_required"
    | "rate_limited"
    | "access_denied"
    | "consent_required"
    | "unstable_render"
    | "unsupported_browser_flow";

/** Snapshot shape consumed by `HttpBrowserProvider` in `document.rs`. */
export interface BrowserSnapshot {
    requested_url: string;
    final_url: string;
    document_html: string;
    captured_at_epoch: number;
    browser_engine: string;
    profile_id: string;
    challenge_state: ChallengeState;
}

/** Inputs accepted by one capture. */
export interface CaptureOptions {
    /** Absolute `http:` or `https:` page URL. */
    url: string;
    /** Per-page navigation budget in milliseconds. */
    timeoutMs?: number;
    /** DOM settle budget after the document reaches `complete`. */
    settleMs?: number;
    /** Allow loopback and private-network targets. Off by default. */
    allowPrivate?: boolean;
}

/** Normalize bracketed IPv6 literals to their bare form. */
function bareHost(hostname: string): string {
    return hostname.startsWith("[") && hostname.endsWith("]") ? hostname.slice(1, -1) : hostname;
}

/** Classify IPv4 literals that must never be reached without `allowPrivate`. */
function isPrivateIpv4(address: string): boolean {
    const octets = address.split(".").map(Number);
    if (octets.length !== 4 || octets.some((octet) => !Number.isInteger(octet) || octet < 0 || octet > 255)) {
        return false;
    }
    const [first, second] = octets;
    if (first === 0 || first === 10 || first === 127) return true;
    if (first === 100 && second >= 64 && second <= 127) return true;
    if (first === 169 && second === 254) return true;
    if (first === 172 && second >= 16 && second <= 31) return true;
    if (first === 192 && second === 168) return true;
    if (first === 198 && (second === 18 || second === 19)) return true;
    return false;
}

/** Classify loopback, link-local and unique-local hosts. */
function isPrivateHost(hostname: string): boolean {
    const host = bareHost(hostname).toLowerCase();
    if (host === "localhost" || host.endsWith(".localhost")) return true;
    if (host === "::1" || host === "0:0:0:0:0:0:0:1") return true;
    if (host.startsWith("::ffff:")) return isPrivateIpv4(host.slice("::ffff:".length));
    const compressed = host.split("%")[0];
    if (compressed.startsWith("fc") || compressed.startsWith("fd")) return true;
    if (/^fe[89ab]/.test(compressed)) return true;
    if (compressed.includes(":")) return false;
    return isPrivateIpv4(compressed);
}

/** Reject non-navigable or disallowed targets before any browser work starts. */
async function assertNavigableUrl(raw: string, allowPrivate: boolean): Promise<URL> {
    let url: URL;
    try {
        url = new URL(raw);
    } catch {
        throw new BrowserError("pandark.browser.request-invalid", "url is not absolute");
    }
    if (url.protocol !== "http:" && url.protocol !== "https:") {
        throw new BrowserError("pandark.browser.request-invalid", `scheme ${url.protocol} is not navigable`);
    }
    if (allowPrivate) return url;

    const host = bareHost(url.hostname);
    if (isPrivateHost(host)) {
        throw new BrowserError("pandark.browser.private-address-denied", host);
    }
    if (host.includes(":") || isPrivateIpv4(host)) return url;

    let addresses: string[];
    try {
        addresses = (await lookup(host, { all: true })).map((record) => record.address);
    } catch {
        // A missing record is a navigation failure, not a policy denial.
        return url;
    }
    for (const address of addresses) {
        if (isPrivateHost(address)) {
            throw new BrowserError("pandark.browser.private-address-denied", `${host} resolves to ${address}`);
        }
    }
    return url;
}

/** Map a main-document status onto a challenge state. */
function challengeStateForStatus(status: number | undefined): ChallengeState {
    switch (status) {
        case 401:
            return "login_required";
        case 403:
            return "access_denied";
        case 429:
            return "rate_limited";
        default:
            return "normal";
    }
}

/** Detect Playwright timeouts without depending on its error classes. */
function isTimeoutError(error: unknown): boolean {
    return error instanceof Error && (error.name === "TimeoutError" || error.message.toLowerCase().includes("timeout"));
}

/** Wait for a stable DOM: N animation frames with no mutation records. */
const SETTLE_PROBE = new Function(
    "args",
    `
    const { quietFrames, timeoutMs } = args;
    return new Promise((resolve) => {
        let quiet = 0;
        let settled = false;
        const finish = (value) => {
            if (settled) return;
            settled = true;
            observer.disconnect();
            clearTimeout(timer);
            resolve(value);
        };
        const observer = new MutationObserver(() => {
            quiet = 0;
        });
        observer.observe(document.documentElement, {
            subtree: true,
            childList: true,
            attributes: true,
            characterData: true,
        });
        const timer = setTimeout(() => finish(false), timeoutMs);
        const frame = () => {
            quiet += 1;
            if (quiet >= quietFrames) {
                finish(true);
                return;
            }
            requestAnimationFrame(frame);
        };
        requestAnimationFrame(frame);
    });
    `,
) as (args: { quietFrames: number, timeoutMs: number }) => Promise<boolean>;

/** Deepest element nesting reached from the document root. */
const DEPTH_PROBE = new Function(
    `
    let deepest = 0;
    const stack = [[document.documentElement, 1]];
    while (stack.length > 0) {
        const entry = stack.pop();
        if (!entry) continue;
        const node = entry[0];
        const depth = entry[1];
        if (depth > deepest) deepest = depth;
        for (const child of node.children) stack.push([child, depth + 1]);
    }
    return deepest;
    `,
) as () => number;

/** Single-process Playwright renderer with hard capture and queue bounds. */
export class BrowserRenderer {
    private browserPromise: Promise<Browser> | null = null;

    private active = 0;

    private pending = 0;

    private readonly waiters: Array<() => void> = [];

    /** Capture one page as a `BrowserSnapshot`. */
    async capture(options: CaptureOptions): Promise<BrowserSnapshot> {
        const url = await assertNavigableUrl(options.url, options.allowPrivate === true);
        const release = await this.acquire();
        try {
            return await this.render(url, options);
        } finally {
            release();
        }
    }

    /** Close the shared browser instance. */
    async close(): Promise<void> {
        const pending = this.browserPromise;
        this.browserPromise = null;
        if (!pending) return;
        const browser = await pending.catch(() => null);
        await browser?.close();
    }

    private acquire(): Promise<() => void> {
        if (this.active < BROWSER_LIMITS.maxConcurrentCaptures) {
            this.active += 1;
            return Promise.resolve(() => this.release());
        }
        if (this.pending >= BROWSER_LIMITS.maxQueuedCaptures) {
            return Promise.reject(new BrowserError("pandark.browser.overloaded", "capture queue is full"));
        }
        this.pending += 1;
        return new Promise((resolve) => {
            this.waiters.push(() => resolve(() => this.release()));
        });
    }

    private release(): void {
        const next = this.waiters.shift();
        if (next) {
            this.pending -= 1;
            next();
            return;
        }
        this.active -= 1;
    }

    private browser(): Promise<Browser> {
        if (!this.browserPromise) {
            this.browserPromise = chromium.launch({ headless: true }).catch((error: unknown) => {
                this.browserPromise = null;
                throw new BrowserError(
                    "pandark.browser.unavailable",
                    error instanceof Error ? error.message : String(error),
                );
            });
        }
        return this.browserPromise;
    }

    private async render(url: URL, options: CaptureOptions): Promise<BrowserSnapshot> {
        const timeoutMs = clampPageTimeout(options.timeoutMs);
        const settleMs = clampSettle(options.settleMs);
        const allowPrivate = options.allowPrivate === true;
        const deadline = Date.now() + timeoutMs;

        let browser: Browser;
        try {
            browser = await this.browser();
        } catch (error) {
            if (error instanceof BrowserError) throw error;
            throw new BrowserError("pandark.browser.unavailable", error instanceof Error ? error.message : String(error));
        }

        let context: BrowserContext | null = null;
        try {
            context = await browser.newContext({
                javaScriptEnabled: true,
                serviceWorkers: "block",
                viewport: { ...BROWSER_LIMITS.viewport },
            });
            if (!allowPrivate) {
                await context.route("**/*", async (route) => {
                    const requestUrl = route.request().url();
                    try {
                        const parsed = new URL(requestUrl);
                        if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
                            await route.abort("blockedbyclient");
                            return;
                        }
                        if (isPrivateHost(parsed.hostname)) {
                            await route.abort("blockedbyclient");
                            return;
                        }
                    } catch {
                        await route.abort("blockedbyclient");
                        return;
                    }
                    await route.continue();
                });
            }

            const page = await context.newPage();
            const state = this.watchPage(page);
            const remaining = Math.max(1, deadline - Date.now());

            let settleFailed = false;
            try {
                await page.goto(url.href, { waitUntil: "load", timeout: remaining });
                await page.waitForFunction(
                    () => document.readyState === "complete",
                    undefined,
                    { timeout: Math.max(1, deadline - Date.now()) },
                );
            } catch (error) {
                if (state.networkBytes > BROWSER_LIMITS.maxNetworkBytes) {
                    throw new BrowserError("pandark.browser.page-budget", `${state.networkBytes} bytes counted`);
                }
                if (state.redirects > BROWSER_LIMITS.maxRedirects) {
                    throw new BrowserError("pandark.browser.redirect-limit", `${state.redirects} redirects`);
                }
                if (isTimeoutError(error)) {
                    throw new BrowserError("pandark.browser.render-timeout", `${timeoutMs} ms budget exhausted`);
                }
                throw new BrowserError(
                    "pandark.browser.navigate-failed",
                    error instanceof Error ? error.message : String(error),
                );
            }

            try {
                const settled = await page.evaluate(SETTLE_PROBE, {
                    quietFrames: BROWSER_LIMITS.quietFrames,
                    timeoutMs: settleMs,
                });
                settleFailed = settled !== true;
            } catch (error) {
                throw new BrowserError(
                    "pandark.browser.navigate-failed",
                    `settle probe failed: ${error instanceof Error ? error.message : String(error)}`,
                );
            }

            if (state.networkBytes > BROWSER_LIMITS.maxNetworkBytes) {
                throw new BrowserError("pandark.browser.page-budget", `${state.networkBytes} bytes counted`);
            }
            if (state.redirects > BROWSER_LIMITS.maxRedirects) {
                throw new BrowserError("pandark.browser.redirect-limit", `${state.redirects} redirects`);
            }

            const depth = await page.evaluate(DEPTH_PROBE);
            if (typeof depth === "number" && depth > BROWSER_LIMITS.maxDomDepth) {
                throw new BrowserError("pandark.browser.dom-depth-limit", `${depth} levels`);
            }

            const html = await page.content();
            const bytes = Buffer.byteLength(html, "utf8");
            if (bytes > BROWSER_LIMITS.maxSnapshotBytes) {
                throw new BrowserError("pandark.browser.snapshot-too-large", `${bytes} bytes`);
            }

            return {
                requested_url: url.href,
                final_url: page.url(),
                document_html: html,
                captured_at_epoch: Math.floor(Date.now() / 1000),
                browser_engine: `chromium-${browser.version().split(".")[0]}`,
                profile_id: "playwright-headless",
                challenge_state: settleFailed ? "unstable_render" : challengeStateForStatus(state.mainStatus),
            };
        } catch (error) {
            if (error instanceof BrowserError) throw error;
            throw new BrowserError("pandark.browser.unavailable", error instanceof Error ? error.message : String(error));
        } finally {
            await context?.close().catch(() => undefined);
        }
    }

    private watchPage(page: Page): { networkBytes: number; redirects: number; mainStatus?: number } {
        const state: { networkBytes: number; redirects: number; mainStatus?: number } = {
            networkBytes: 0,
            redirects: 0,
        };
        page.on("response", (response) => {
            const declared = Number(response.headers()["content-length"] ?? 0);
            if (Number.isFinite(declared) && declared > 0) state.networkBytes += declared;
            if (response.request().isNavigationRequest() && response.request().resourceType() === "document") {
                if (response.status() >= 300 && response.status() < 400) {
                    state.redirects += 1;
                } else if (state.mainStatus === undefined) {
                    state.mainStatus = response.status();
                }
            }
        });
        return state;
    }
}
