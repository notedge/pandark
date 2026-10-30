import type { PandarkBindings } from "../types.js";
import { loadPandarkNode } from "../node/load.js";

export type CliContext = {
    bindings: PandarkBindings | null;
    toolVersion: string;
};

export function loadBindings(): PandarkBindings | null {
    try {
        return loadPandarkNode();
    } catch {
        return null;
    }
}

export function createContext(): CliContext {
    const bindings = loadBindings();
    return {
        bindings,
        toolVersion: bindings?.pandarkVersion() ?? "0.0.0",
    };
}
