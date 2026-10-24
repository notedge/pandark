export type MarkdownResult = {
    markdown: string;
    diagnosticNamespace: string;
    diagnosticCodes: string[];
};

/** One fetched page returned by the Rust crawl runtime. */
export type CrawlPageArtifact = {
    requestedUrl: string;
    finalUrl: string;
    status: number;
    contentType?: string;
    headersJson: string;
    body: Uint8Array;
};

export type PandarkBindings = {
    pandarkVersion: () => string;
    planCrawl: (
        seed: string,
        maxDepth?: number,
        maxRequests?: number,
    ) => {
        frontier: string[];
        reportJson: string;
    };
    extractFile: (
        inputPath: string,
        sourceUrl?: string,
    ) => {
        exitCode: number;
        status: string;
        documentJson?: string;
        reportJson: string;
    };
    extractInput: (
        inputPath: string,
        from?: string,
        sourceUrl?: string,
        challengePolicy?: string,
    ) => {
        exitCode: number;
        status: string;
        documentJson?: string;
        reportJson: string;
    };
    crawlFile: (
        seed: string,
        maxDepth?: number,
        maxRequests?: number,
        browserFixturesDir?: string,
        browserFallback?: string,
        challengePolicy?: string,
        browserEndpoint?: string,
        cacheDir?: string,
        scopeHosts?: string[],
        scopeDomains?: string[],
        politeness?: string,
    ) => {
        exitCode: number;
        reportJson: string;
        committedPages: string[];
        checkpointJson?: string;
        pages: CrawlPageArtifact[];
    };
    resumeCrawlFile: (
        checkpointJson: string,
        browserFixturesDir?: string,
        browserFallback?: string,
        challengePolicy?: string,
        browserEndpoint?: string,
        cacheDir?: string,
    ) => {
        exitCode: number;
        reportJson: string;
        committedPages: string[];
        checkpointJson?: string;
        pages: CrawlPageArtifact[];
    };
    fetchSeed: (seed: string) => {
        exitCode: number;
        artifactJson: string;
    };
    inspectFile: (
        inputPath: string,
        stage?: string,
    ) => {
        exitCode: number;
        reportJson: string;
    };
    inspectInput: (
        inputPath: string,
        stage?: string,
        from?: string,
        challengePolicy?: string,
    ) => {
        exitCode: number;
        reportJson: string;
    };
    /** One-shot: HTML bytes → Markdown string via Notedown IR. */
    extractToMarkdown: (
        htmlBytes: Uint8Array,
        sourceUrl?: string,
    ) => MarkdownResult;
};
