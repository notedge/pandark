import assert from "node:assert/strict";
import test from "node:test";

type ExportSurface = Record<string, unknown>;

const shapeOf = (value: unknown): string => {
    if (typeof value === "function") {
        const asyncMarker = value.constructor.name === "AsyncFunction" ? "async " : "";
        return `${asyncMarker}function/${value.length}`;
    }
    if (Array.isArray(value)) {
        return `array/${value.length}`;
    }
    return typeof value;
};

const describe = (surface: ExportSurface): Record<string, string> => {
    const described: Record<string, string> = {};
    for (const name of Object.keys(surface).sort()) {
        described[name] = shapeOf(surface[name]);
    }
    return described;
};

/** The three difference classes the gate reports: node-only, wasm-only, and names present on both with different shapes. */
function diffSurfaces(node: Record<string, string>, wasm: Record<string, string>): {
    nodeOnly: string[];
    wasmOnly: string[];
    mismatched: Array<{ name: string; node: string; wasm: string }>;
} {
    const nodeNames = new Set(Object.keys(node));
    const wasmNames = new Set(Object.keys(wasm));
    return {
        nodeOnly: [...nodeNames].filter((name) => !wasmNames.has(name)).sort(),
        wasmOnly: [...wasmNames].filter((name) => !nodeNames.has(name)).sort(),
        mismatched: [...nodeNames]
            .filter((name) => wasmNames.has(name) && node[name] !== wasm[name])
            .sort()
            .map((name) => ({ name, node: node[name], wasm: wasm[name] })),
    };
}

test("G-ARCH-5 node and wasm entries only differ on their runtime adapter surface", async () => {
    const nodeEntry = (await import("../src/node/index.js")) as ExportSurface;
    const wasmEntry = (await import("../src/wasm/index.js")) as ExportSurface;

    // Intentional differences: the node entry may only expose the native carrier loader, while
    // the wasm entry exposes its loader plus the instantiation primitives the portable artifact needs.
    // Any new export on one side, or a shape change on a shared name, fails this gate.
    const expected = {
        nodeOnly: ["loadPandarkNode"],
        wasmOnly: [
            "PandarkWasmCapabilityError",
            "WASM_CAPABILITIES",
            "instantiatePandarkWasm",
            "loadPandarkWasm",
            "locatePandarkWasm",
        ],
        mismatched: [],
    };

    assert.deepEqual(diffSurfaces(describe(nodeEntry), describe(wasmEntry)), expected);
});
