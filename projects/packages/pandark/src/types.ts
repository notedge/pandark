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
    ) => {
        exitCode: number;
        reportJson: string;
        committedPages: string[];
        checkpointJson?: string;
    };
    resumeCrawlFile: (
        checkpointJson: string,
        browserFixturesDir?: string,
        browserFallback?: string,
        challengePolicy?: string,
    ) => {
        exitCode: number;
        reportJson: string;
        committedPages: string[];
        checkpointJson?: string;
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
};
