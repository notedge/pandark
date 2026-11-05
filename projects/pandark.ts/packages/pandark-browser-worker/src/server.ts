import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";

import { BrowserRenderer, type CaptureOptions } from "./capture.js";
import { BrowserError } from "./limits.js";

/** Worker server settings. */
export interface BrowserWorkerOptions {
    /** Allow loopback and private-network targets. Off by default. */
    allowPrivate?: boolean;
    /** Per-page navigation budget passed to every capture. */
    timeoutMs?: number;
    /** DOM settle budget passed to every capture. */
    settleMs?: number;
}

/** Running worker handle. */
export interface BrowserWorker {
    server: Server;
    renderer: BrowserRenderer;
    /** Close the HTTP server and the shared browser. */
    close: () => Promise<void>;
}

function sendJson(response: ServerResponse, status: number, payload: unknown, code?: string): void {
    const body = JSON.stringify(payload);
    const headers: Record<string, string> = { "content-type": "application/json; charset=utf-8" };
    if (code) headers["x-pandark-code"] = code;
    response.writeHead(status, headers);
    response.end(body);
}

/** Build the HTTP worker that implements the existing browser endpoint contract. */
export function createBrowserWorkerServer(options: BrowserWorkerOptions = {}): BrowserWorker {
    const renderer = new BrowserRenderer();

    const handle = async (request: IncomingMessage, response: ServerResponse): Promise<void> => {
        const method = request.method ?? "GET";
        const url = new URL(request.url ?? "/", "http://worker.local");

        if (method !== "GET") {
            sendJson(response, 405, { code: "pandark.browser.request-invalid", message: "GET only" }, "pandark.browser.request-invalid");
            return;
        }
        if (url.pathname === "/health") {
            sendJson(response, 200, { ok: true, engine: "playwright-chromium" });
            return;
        }
        if (url.pathname !== "/") {
            sendJson(response, 404, { code: "pandark.browser.request-invalid", message: "unknown path" }, "pandark.browser.request-invalid");
            return;
        }

        const target = url.searchParams.get("url");
        if (!target) {
            sendJson(response, 400, { code: "pandark.browser.request-invalid", message: "missing url" }, "pandark.browser.request-invalid");
            return;
        }

        const captureOptions: CaptureOptions = {
            url: target,
            timeoutMs: options.timeoutMs,
            settleMs: options.settleMs,
            allowPrivate: options.allowPrivate,
        };
        try {
            const snapshot = await renderer.capture(captureOptions);
            sendJson(response, 200, snapshot);
        } catch (error) {
            if (error instanceof BrowserError) {
                sendJson(response, error.status, { code: error.code, message: error.message }, error.code);
                return;
            }
            throw error;
        }
    };

    const server = createServer((request, response) => {
        void handle(request, response).catch((error: unknown) => {
            const code = error instanceof BrowserError ? error.code : "pandark.browser.unavailable";
            const status = error instanceof BrowserError ? error.status : 503;
            sendJson(response, status, { code, message: String(error) }, code);
        });
    });

    return {
        server,
        renderer,
        close: async () => {
            await new Promise<void>((resolve) => {
                server.close(() => resolve());
            });
            await renderer.close();
        },
    };
}
