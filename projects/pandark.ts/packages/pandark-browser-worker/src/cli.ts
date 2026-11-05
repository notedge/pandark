import { createBrowserWorkerServer } from "./server.js";

export const DEFAULT_HOST = "127.0.0.1";
export const DEFAULT_PORT = 9333;

/** Parse `--flag value` pairs used by the worker CLI. */
function optionValue(args: string[], flag: string): string | undefined {
    const index = args.indexOf(flag);
    return index >= 0 ? args[index + 1] : undefined;
}

/** Run the worker CLI and resolve with a process exit code. */
export async function runCli(args: string[]): Promise<number> {
    if (args.includes("--help") || args.includes("-h")) {
        process.stdout.write(`pandark-browser-worker

Usage:
  pandark-browser-worker [--host 127.0.0.1] [--port 9333] [--allow-private]
                         [--timeout-ms 20000] [--settle-ms 5000]

Serves GET /?url=<page> as a BrowserSnapshot JSON document over the existing
Pandark browser endpoint contract. Rendering uses bundled Playwright Chromium
in headless mode and refuses loopback or private-network targets unless
--allow-private is set.`);
        return 0;
    }

    const host = optionValue(args, "--host") ?? DEFAULT_HOST;
    const port = Number(optionValue(args, "--port") ?? DEFAULT_PORT);
    if (!Number.isInteger(port) || port < 0 || port > 65_535) {
        process.stderr.write(`invalid --port: ${optionValue(args, "--port")}\n`);
        return 2;
    }
    const timeoutValue = optionValue(args, "--timeout-ms");
    const settleValue = optionValue(args, "--settle-ms");

    const worker = createBrowserWorkerServer({
        allowPrivate: args.includes("--allow-private"),
        timeoutMs: timeoutValue === undefined ? undefined : Number(timeoutValue),
        settleMs: settleValue === undefined ? undefined : Number(settleValue),
    });

    await new Promise<void>((resolve, reject) => {
        worker.server.once("error", reject);
        worker.server.listen(port, host, resolve);
    });

    const address = worker.server.address();
    const bound = typeof address === "object" && address ? `http://${host}:${address.port}` : `http://${host}:${port}`;
    process.stdout.write(`pandark-browser-worker listening on ${bound}\n`);

    const shutdown = async (): Promise<void> => {
        await worker.close();
        process.exit(0);
    };
    process.once("SIGINT", () => void shutdown());
    process.once("SIGTERM", () => void shutdown());
    return 0;
}
