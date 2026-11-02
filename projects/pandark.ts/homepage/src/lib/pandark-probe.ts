export type PandarkProbe = {
    version: string;
    backend: 'node' | 'browser-unavailable';
    platformPackage: string;
    commands: string[];
};

/** Browser hosts cannot load Node-API bindings; surface an explicit unavailable state. */
export function probePandarkBrowserUnavailable(): PandarkProbe {
    return {
        version: '(Node binding required)',
        backend: 'browser-unavailable',
        platformPackage: '@notedge/pandark + optional @notedge/pandark-<platform>',
        commands: ['extract()', 'plan()', 'crawl()', 'inspect()'],
    };
}

/**
 * Homepage runs in the browser, so the probe is always the unavailable snapshot.
 * The client bundle must not reference the Node-API package: the assembler packs
 * every statically visible import for the browser, and `@notedge/pandark` is a
 * Node-only TS-source package that can never execute in a browser host.
 */
export async function probePandark(): Promise<PandarkProbe> {
    return probePandarkBrowserUnavailable();
}
