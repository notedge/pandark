export type PandarkProbe = {
    version: string;
    backend: 'node' | 'browser-unavailable';
    platformPackage: string;
    commands: string[];
};

const PLATFORM_PACKAGES: Record<string, string> = {
    'win32-x64': '@notedge/pandark-win32-x64',
    'linux-x64': '@notedge/pandark-linux-x64',
    'linux-arm64': '@notedge/pandark-linux-arm64',
    'darwin-x64': '@notedge/pandark-darwin-x64',
    'darwin-arm64': '@notedge/pandark-darwin-arm64',
};

function listPublicCommands(client: {
    version?: () => string;
    plan?: unknown;
    extract?: unknown;
    crawl?: unknown;
    resume?: unknown;
    fetch?: unknown;
    inspect?: unknown;
}): string[] {
    const commands: string[] = [];
    if (client.version) commands.push('version()');
    if (client.plan) commands.push('plan()');
    if (client.extract) commands.push('extract()');
    if (client.crawl) commands.push('crawl()');
    if (client.resume) commands.push('resume()');
    if (client.fetch) commands.push('fetch()');
    if (client.inspect) commands.push('inspect()');
    return commands;
}

/** Node probe for CI and local verify without a browser. */
export async function probePandarkNode(): Promise<PandarkProbe> {
    const { createPandark } = await import('@notedge/pandark');
    const pandark = createPandark();
    const key = `${process.platform}-${process.arch}`;
    return {
        version: pandark.version(),
        backend: 'node',
        platformPackage: PLATFORM_PACKAGES[key] ?? '(unsupported host)',
        commands: listPublicCommands(pandark),
    };
}

/** Browser hosts cannot load Node-API bindings; surface an explicit unavailable state. */
export function probePandarkBrowserUnavailable(): PandarkProbe {
    return {
        version: '(Node binding required)',
        backend: 'browser-unavailable',
        platformPackage: '@notedge/pandark + optional @notedge/pandark-<platform>',
        commands: ['extract()', 'plan()', 'crawl()', 'inspect()'],
    };
}

/** Prefer Node probe on the server; browsers get an honest unavailable snapshot. */
export async function probePandark(): Promise<PandarkProbe> {
    if (typeof window !== 'undefined') {
        return probePandarkBrowserUnavailable();
    }
    return probePandarkNode();
}
