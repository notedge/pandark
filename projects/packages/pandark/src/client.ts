import { loadPandarkNode } from "./node/load.js";
import type { PandarkBindings } from "./types.js";

export type PandarkExtractOptions = {
    from?: string;
    sourceUrl?: string;
    challengePolicy?: string;
};

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
    version(): string;
    plan(seed: string, depth?: number, budget?: number): ReturnType<PandarkBindings["planCrawl"]>;
    fetch(seed: string): ReturnType<PandarkBindings["fetchSeed"]>;
    extract(inputPath: string, options?: PandarkExtractOptions): ReturnType<PandarkBindings["extractInput"]>;
    inspect(inputPath: string, stage?: string, options?: Pick<PandarkExtractOptions, "from" | "challengePolicy">): ReturnType<PandarkBindings["inspectInput"]>;
    crawl(seed: string, options?: PandarkCrawlOptions): ReturnType<PandarkBindings["crawlFile"]>;
    resume(checkpointJson: string, options?: Omit<PandarkCrawlOptions, "depth" | "budget">): ReturnType<PandarkBindings["resumeCrawlFile"]>;
};

/** High-level Pandark client. Loads native bindings once and exposes crawl/extract operations. */
export function createPandark(): PandarkClient {
    const binding = loadPandarkNode();
    return {
        version: () => binding.pandarkVersion(),
        plan: (seed, depth, budget) => binding.planCrawl(seed, depth, budget),
        fetch: (seed) => binding.fetchSeed(seed),
        extract: (inputPath, options) =>
            binding.extractInput(
                inputPath,
                options?.from,
                options?.sourceUrl,
                options?.challengePolicy,
            ),
        inspect: (inputPath, stage, options) =>
            binding.inspectInput(inputPath, stage, options?.from, options?.challengePolicy),
        crawl: (seed, options) =>
            binding.crawlFile(
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
            binding.resumeCrawlFile(
                checkpointJson,
                options?.browserFixturesDir,
                options?.browserFallback,
                options?.challengePolicy,
                options?.browserEndpoint,
                options?.cacheDir,
            ),
    };
}
