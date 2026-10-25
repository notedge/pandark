import { loadPandarkNode } from "./node/load.js";
import type { PandarkBindings } from "./types.js";

export type PandarkCrawlOptions = {
    depth?: number;
    budget?: number;
    browserFixturesDir?: string;
    browserFallback?: string;
    challengePolicy?: string;
    browserEndpoint?: string;
    cacheDir?: string;
};

export type PandarkClient = {
    version: () => string;
    plan: (
        seed: string,
        depth?: number,
        budget?: number,
    ) => {
        frontier: string[];
        reportJson: string;
    };
    fetch: (seed: string) => Promise<{
        exitCode: number;
        artifactJson: string;
    }>;
    crawl: (
        seed: string,
        options?: PandarkCrawlOptions,
    ) => Promise<{
        exitCode: number;
        reportJson: string;
        committedPages: string[];
        checkpointJson?: string;
    }>;
    resume: (
        checkpointJson: string,
        options?: Omit<PandarkCrawlOptions, "depth" | "budget">,
    ) => Promise<{
        exitCode: number;
        reportJson: string;
        committedPages: string[];
        checkpointJson?: string;
    }>;
    extract: (
        inputPath: string,
        options?: {
            from?: string;
            sourceUrl?: string;
            challengePolicy?: string;
        },
    ) => Promise<{
        exitCode: number;
        status: string;
        documentJson?: string;
        reportJson: string;
    }>;
    inspect: (
        inputPath: string,
        stage?: string,
        options?: {
            from?: string;
            challengePolicy?: string;
        },
    ) => {
        exitCode: number;
        reportJson: string;
    };
};

/** High-level Pandark client backed by the platform Node-API binding. */
export function createPandark(bindings?: PandarkBindings): PandarkClient {
    const native = bindings ?? loadPandarkNode();

    return {
        version: () => native.pandarkVersion(),
        plan: (seed, depth, budget) => native.planCrawl(seed, depth, budget),
        fetch: (seed) => native.fetchSeed(seed),
        crawl: (seed, options) =>
            native.crawlFile(
                seed,
                options?.depth,
                options?.budget,
                options?.browserFixturesDir,
                options?.browserFallback,
                options?.challengePolicy,
                options?.browserEndpoint,
                options?.cacheDir,
            ),
        resume: (checkpointJson, options) =>
            native.resumeCrawlFile(
                checkpointJson,
                options?.browserFixturesDir,
                options?.browserFallback,
                options?.challengePolicy,
                options?.browserEndpoint,
                options?.cacheDir,
            ),
        extract: (inputPath, options) => {
            if (options?.from || options?.challengePolicy) {
                return native.extractInput(
                    inputPath,
                    options.from,
                    options.sourceUrl,
                    options.challengePolicy,
                );
            }
            return native.extractFile(inputPath, options?.sourceUrl);
        },
        inspect: (inputPath, stage, options) => {
            if (options?.from || options?.challengePolicy) {
                return native.inspectInput(
                    inputPath,
                    stage,
                    options.from,
                    options.challengePolicy,
                );
            }
            return native.inspectFile(inputPath, stage);
        },
    };
}
