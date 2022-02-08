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

function listPublicCommands(binding: {
    pandarkVersion?: () => string;
    planCrawl?: unknown;
    extractInput?: unknown;
    crawlFile?: unknown;
    resumeCrawlFile?: unknown;
    fetchSeed?: unknown;
    inspectInput?: unknown;
}): string[] {
    const commands: string[] = [];
    if (binding.pandarkVersion) commands.push('pandarkVersion()');
    if (binding.planCrawl) commands.push('planCrawl()');
    if (binding.extractInput) commands.push('extractInput()');
    if (binding.crawlFile) commands.push('crawlFile()');
    if (binding.resumeCrawlFile) commands.push('resumeCrawlFile()');
    if (binding.fetchSeed) commands.push('fetchSeed()');
    if (binding.inspectInput) commands.push('inspectInput()');
    return commands;
}

/** Node probe for CI and local verify without a browser. */
export async function probePandarkNode(): Promise<PandarkProbe> {
    const { loadPandarkNode } = await import('@notedge/pandark/node');
    const binding = loadPandarkNode();
    const key = `${process.platform}-${process.arch}`;
    return {
        version: binding.pandarkVersion(),
        backend: 'node',
        platformPackage: PLATFORM_PACKAGES[key] ?? '(unsupported host)',
        commands: listPublicCommands(binding),
    };
}

/** Browser hosts cannot load Node-API bindings; surface an explicit unavailable state. */
export function probePandarkBrowserUnavailable(): PandarkProbe {
    return {
        version: '(Node binding required)',
        backend: 'browser-unavailable',
        platformPackage: '@notedge/pandark + optional @notedge/pandark-<platform>',
        commands: ['extractInput()', 'planCrawl()', 'crawlFile()', 'inspectInput()'],
    };
}

/** Prefer Node probe on the server; browsers get an honest unavailable snapshot. */
export async function probePandark(): Promise<PandarkProbe> {
    if (typeof window !== 'undefined') {
        return probePandarkBrowserUnavailable();
    }
    return probePandarkNode();
}
