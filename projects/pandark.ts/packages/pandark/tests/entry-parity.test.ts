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

/** The three difference classes between an earlier surface and a later one. */
function diffSurfaces(
    earlier: Record<string, string>,
    later: Record<string, string>,
): { earlierOnly: string[]; laterOnly: string[]; mismatched: Array<{ name: string; earlier: string; later: string }> } {
    const earlierNames = new Set(Object.keys(earlier));
    const laterNames = new Set(Object.keys(later));
    return {
        earlierOnly: [...earlierNames].filter((name) => !laterNames.has(name)).sort(),
        laterOnly: [...laterNames].filter((name) => !earlierNames.has(name)).sort(),
        mismatched: [...earlierNames]
            .filter((name) => laterNames.has(name) && earlier[name] !== later[name])
            .sort()
            .map((name) => ({ name, earlier: earlier[name], later: later[name] })),
    };
}

// Runtime adapter surfaces. Platform entries own these names. The root entry is the
// authoring/domain surface plus exactly one documented integrator loader re-export
// (`loadPandarkNode`, readme line 47); it must never re-expose the wasm primitives.
const NODE_RUNTIME_EXPORTS = ["loadPandarkNode"];
const WASM_RUNTIME_EXPORTS = [
    "PandarkWasmCapabilityError",
    "WASM_CAPABILITIES",
    "instantiatePandarkWasm",
    "loadPandarkWasm",
    "locatePandarkWasm",
];
const ROOT_DOCUMENTED_RUNTIME_REEXPORTS = ["loadPandarkNode"];

test("G-ARCH-5 root, node and wasm entries share no runtime loader and drift in three classes", async () => {
    const rootEntry = (await import("../src/index.js")) as ExportSurface;
    const nodeEntry = (await import("../src/node/index.js")) as ExportSurface;
    const wasmEntry = (await import("../src/wasm/index.js")) as ExportSurface;

    const root = describe(rootEntry);
    const node = describe(nodeEntry);
    const wasm = describe(wasmEntry);

    // Root vs node: root may only duplicate the documented integrator loader, with the same shape.
    const rootNode = diffSurfaces(root, node);
    assert.deepEqual(
        { nodeOnly: rootNode.laterOnly, mismatched: rootNode.mismatched },
        { nodeOnly: [], mismatched: [] },
    );
    assert.ok(rootNode.earlierOnly.length > 0, "root keeps its own authoring surface");

    // Root vs wasm: the wasm primitives are never re-exported from root.
    const rootWasm = diffSurfaces(root, wasm);
    assert.deepEqual(
        { wasmOnly: rootWasm.laterOnly, mismatched: rootWasm.mismatched },
        { wasmOnly: WASM_RUNTIME_EXPORTS, mismatched: [] },
    );

    // Node vs wasm: the two runtime entries stay disjoint, with no shape drift on shared names.
    const nodeWasm = diffSurfaces(node, wasm);
    assert.deepEqual(nodeWasm, { earlierOnly: NODE_RUNTIME_EXPORTS, laterOnly: WASM_RUNTIME_EXPORTS, mismatched: [] });

    // Mechanical predicate: the root entry may duplicate only the documented integrator loader.
    const runtimeNames = new Set([...NODE_RUNTIME_EXPORTS, ...WASM_RUNTIME_EXPORTS]);
    const collisions = Object.keys(root)
        .filter((name) => runtimeNames.has(name))
        .sort();
    assert.deepEqual(collisions, ROOT_DOCUMENTED_RUNTIME_REEXPORTS, "root must not re-expose undocumented runtime loaders");
});
