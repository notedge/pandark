/**
 * Platform carriers that may ship `lib/pandark_wasm.wasm`.
 *
 * The artifact is portable, but the package contract keeps one copy inside
 * every carrier so a consumer never reaches for another platform's package.
 */
const PLATFORM_PACKAGES: Record<string, string> = {
    "win32-x64": "@notedge/pandark-win32-x64",
    "linux-x64": "@notedge/pandark-linux-x64",
    "linux-arm64": "@notedge/pandark-linux-arm64",
    "darwin-x64": "@notedge/pandark-darwin-x64",
    "darwin-arm64": "@notedge/pandark-darwin-arm64",
};

/** Capabilities the host must inject through the fixture document. */
export const WASM_CAPABILITIES = ["fetch", "store", "clock", "cancel"] as const;

/** One page snapshot injected by the host. */
export type WasmFixturePage = {
    url: string;
    finalUrl?: string;
    status?: number;
    contentType?: string;
    headers?: Record<string, string>;
    /** Inline UTF-8 body. */
    body: string;
    /** Where the host obtained the bytes. Defaults to `test`. */
    provenance?: "test" | "network";
};

/** Capability document injected into one run. */
export type WasmCapabilities = {
    pages: WasmFixturePage[];
    clock?: number;
    cancel?: boolean;
};

/** Request issued to the adapter. */
export type WasmRunRequest = {
    seeds: string[];
    maxRequests?: number;
};

/** One page outcome returned by the adapter. */
export type WasmPageOutcome = {
    url: string;
    finalUrl: string;
    status: string;
    storeKey?: string;
    stored?: "document" | "fetch_only";
};

/** One store write returned to the host. */
export type WasmStoredRecord = {
    key: string;
    payload: string;
};

/** Result of one capability-driven run. */
export type WasmRunOutcome = {
    reportJson: string;
    pages: WasmPageOutcome[];
    stored: WasmStoredRecord[];
};

/** Structured capability failure raised by the adapter. */
export class PandarkWasmCapabilityError extends Error {
    readonly capability: string;

    constructor(capability: string, message: string) {
        super(`${capability} capability failed: ${message}`);
        this.name = "PandarkWasmCapabilityError";
        this.capability = capability;
    }
}

type WasmExports = {
    memory: WebAssembly.Memory;
    pandark_wasm_abi_version: () => number;
    pandark_wasm_alloc: (length: number) => number;
    pandark_wasm_dealloc: (pointer: number, length: number) => void;
    pandark_wasm_run: (requestPointer: number, requestLength: number, capabilitiesPointer: number, capabilitiesLength: number) => number;
    pandark_wasm_result_ptr: () => number;
    pandark_wasm_result_len: () => number;
    pandark_wasm_version_ptr: () => number;
    pandark_wasm_version_len: () => number;
};

/** Loaded Pandark WebAssembly adapter. */
export type PandarkWasm = {
    readonly abiVersion: number;
    readonly version: string;
    readonly capabilities: readonly string[];
    run: (request: WasmRunRequest, capabilities: WasmCapabilities) => WasmRunOutcome;
};

type Envelope = {
    ok: boolean;
    reportJson?: string;
    pages?: WasmPageOutcome[];
    stored?: WasmStoredRecord[];
    error?: { capability: string; message: string };
};

const encoder = new TextEncoder();
const decoder = new TextDecoder();

type WasmBytes = Uint8Array<ArrayBuffer>;

function bytesOf(source: Uint8Array | ArrayBuffer): WasmBytes {
    return new Uint8Array(source);
}

async function readSource(source: Uint8Array | ArrayBuffer | string | URL): Promise<WasmBytes> {
    if (source instanceof Uint8Array || source instanceof ArrayBuffer) {
        return bytesOf(source);
    }
    const href = String(source);
    if (href.startsWith("http://") || href.startsWith("https://")) {
        if (typeof fetch !== "function") {
            throw new Error(`Cannot fetch \`${href}\`: this host has no fetch capability`);
        }
        const response = await fetch(href);
        if (!response.ok) {
            throw new Error(`Cannot fetch \`${href}\`: HTTP ${response.status}`);
        }
        return new Uint8Array(await response.arrayBuffer());
    }
    const wasmPath = href.startsWith("file:") ? await fileUrlToPath(href) : href;
    const { readFile } = await import("node:fs/promises");
    return new Uint8Array(await readFile(wasmPath));
}

async function fileUrlToPath(href: string): Promise<string> {
    const { fileURLToPath } = await import("node:url");
    return fileURLToPath(href);
}

function decode(memory: WebAssembly.Memory, pointer: number, length: number): string {
    return decoder.decode(new Uint8Array(memory.buffer, pointer, length).slice());
}

function write(memory: WebAssembly.Memory, pointer: number, bytes: Uint8Array): void {
    new Uint8Array(memory.buffer, pointer, bytes.length).set(bytes);
}

function buildAdapter(instance: WebAssembly.Instance): PandarkWasm {
    const exports = instance.exports as unknown as WasmExports;
    const abiVersion = exports.pandark_wasm_abi_version();
    const version = decode(exports.memory, exports.pandark_wasm_version_ptr(), exports.pandark_wasm_version_len());

    return {
        abiVersion,
        version,
        capabilities: [...WASM_CAPABILITIES],
        run: (request, capabilities) => {
            const requestBytes = encoder.encode(JSON.stringify({ seeds: request.seeds, max_requests: request.maxRequests ?? 16 }));
            const capabilityBytes = encoder.encode(
                JSON.stringify({ pages: capabilities.pages, clock: capabilities.clock ?? 0, cancel: capabilities.cancel ?? false }),
            );
            const requestPointer = exports.pandark_wasm_alloc(requestBytes.length);
            const capabilitiesPointer = exports.pandark_wasm_alloc(capabilityBytes.length);
            try {
                write(exports.memory, requestPointer, requestBytes);
                write(exports.memory, capabilitiesPointer, capabilityBytes);
                exports.pandark_wasm_run(requestPointer, requestBytes.length, capabilitiesPointer, capabilityBytes.length);
                const envelope = JSON.parse(
                    decode(exports.memory, exports.pandark_wasm_result_ptr(), exports.pandark_wasm_result_len()),
                ) as Envelope;
                if (!envelope.ok) {
                    throw new PandarkWasmCapabilityError(envelope.error?.capability ?? "unknown", envelope.error?.message ?? "unknown failure");
                }
                return {
                    reportJson: envelope.reportJson ?? "",
                    pages: envelope.pages ?? [],
                    stored: envelope.stored ?? [],
                };
            } finally {
                exports.pandark_wasm_dealloc(requestPointer, requestBytes.length);
                exports.pandark_wasm_dealloc(capabilitiesPointer, capabilityBytes.length);
            }
        },
    };
}

/**
 * Instantiate the adapter from bytes, a local path or an HTTP(S) URL.
 *
 * The module has no imports, so no host import object is needed.
 */
export async function instantiatePandarkWasm(source: Uint8Array | ArrayBuffer | string | URL): Promise<PandarkWasm> {
    const bytes = await readSource(source);
    const module = new WebAssembly.Module(bytes);
    const imports = WebAssembly.Module.imports(module);
    if (imports.length > 0) {
        throw new Error(`pandark wasm must not import anything, got: ${JSON.stringify(imports)}`);
    }
    const instance = await WebAssembly.instantiate(module, {});
    return buildAdapter(instance);
}

/** Path of the wasm artifact shipped by the current platform carrier. */
export async function locatePandarkWasm(): Promise<{ packageName: string; wasmPath: string }> {
    const key = `${process.platform}-${process.arch}`;
    const packageName = PLATFORM_PACKAGES[key];
    if (!packageName) {
        throw new Error(`Unsupported platform for Pandark wasm adapter: ${key}`);
    }
    const { createRequire } = await import("node:module");
    const { dirname, join } = await import("node:path");
    const { existsSync } = await import("node:fs");
    const require = createRequire(import.meta.url);
    const packageJson = require.resolve(`${packageName}/package.json`);
    const wasmPath = join(dirname(packageJson), "lib", "pandark_wasm.wasm");
    if (!existsSync(wasmPath)) {
        throw new Error(`Missing \`${packageName}/lib/pandark_wasm.wasm\`. Run \`pnpm build:wasm\` before publishing`);
    }
    return { packageName, wasmPath };
}

/** Load the adapter from the current platform carrier. */
export async function loadPandarkWasm(): Promise<PandarkWasm> {
    const { wasmPath } = await locatePandarkWasm();
    const { readFile } = await import("node:fs/promises");
    return instantiatePandarkWasm(new Uint8Array(await readFile(wasmPath)));
}
