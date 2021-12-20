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
    crawlFile: (
        seed: string,
        maxDepth?: number,
        maxRequests?: number,
    ) => {
        exitCode: number;
        reportJson: string;
        committedPages: string[];
    };
    fetchSeed: (seed: string) => {
        exitCode: number;
        artifactJson: string;
    };
};
