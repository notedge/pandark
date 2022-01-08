import { createRequire } from "node:module";

import type { PandarkBindings } from "../types.js";

type NativeBinding = {
    pandarkVersion: () => string;
    planCrawl: (seed: string, maxDepth?: number, maxRequests?: number) => {
        frontier: string[];
        reportJson: string;
    };
    extractFile: (inputPath: string, sourceUrl?: string) => {
        exitCode: number;
        status: string;
        documentJson?: string;
        reportJson: string;
    };
    extractInput: (inputPath: string, from?: string, sourceUrl?: string, challengePolicy?: string) => {
        exitCode: number;
        status: string;
        documentJson?: string;
        reportJson: string;
    };
    crawlFile: (seed: string, maxDepth?: number, maxRequests?: number, browserFixturesDir?: string, browserFallback?: string, challengePolicy?: string, browserEndpoint?: string) => {
        exitCode: number;
        reportJson: string;
        committedPages: string[];
        checkpointJson?: string;
    };
    resumeCrawlFile: (checkpointJson: string, browserFixturesDir?: string, browserFallback?: string, challengePolicy?: string, browserEndpoint?: string) => {
        exitCode: number;
        reportJson: string;
        committedPages: string[];
        checkpointJson?: string;
    };
    fetchSeed: (seed: string) => {
        exitCode: number;
        artifactJson: string;
    };
    inspectFile: (inputPath: string, stage?: string) => {
        exitCode: number;
        reportJson: string;
    };
    inspectInput: (inputPath: string, stage?: string, from?: string, challengePolicy?: string) => {
        exitCode: number;
        reportJson: string;
    };
};

const PLATFORM_PACKAGES: Record<string, string> = {
    "win32-x64": "@notedge/pandark-win32-x64",
    "linux-x64": "@notedge/pandark-linux-x64",
    "linux-arm64": "@notedge/pandark-linux-arm64",
    "darwin-x64": "@notedge/pandark-darwin-x64",
    "darwin-arm64": "@notedge/pandark-darwin-arm64",
};

/** Load the platform-specific Node-API binary from `@notedge/pandark-<platform>`. */
export function loadPandarkNode(): PandarkBindings {
    const key = `${process.platform}-${process.arch}`;
    const pkg = PLATFORM_PACKAGES[key];
    if (!pkg) {
        throw new Error(`Unsupported platform for Pandark native bindings: ${key}`);
    }
    const require = createRequire(import.meta.url);
    const binding = require(pkg).default as NativeBinding;
    return {
        pandarkVersion: () => binding.pandarkVersion(),
        planCrawl: (seed, maxDepth, maxRequests) => binding.planCrawl(seed, maxDepth, maxRequests),
        extractFile: (inputPath, sourceUrl) => binding.extractFile(inputPath, sourceUrl),
        extractInput: (inputPath, from, sourceUrl, challengePolicy) =>
            binding.extractInput(inputPath, from, sourceUrl, challengePolicy),
        crawlFile: (seed, maxDepth, maxRequests, browserFixturesDir, browserFallback, challengePolicy, browserEndpoint) =>
            binding.crawlFile(
                seed,
                maxDepth,
                maxRequests,
                browserFixturesDir,
                browserFallback,
                challengePolicy,
                browserEndpoint,
            ),
        resumeCrawlFile: (checkpointJson, browserFixturesDir, browserFallback, challengePolicy, browserEndpoint) =>
            binding.resumeCrawlFile(
                checkpointJson,
                browserFixturesDir,
                browserFallback,
                challengePolicy,
                browserEndpoint,
            ),
        fetchSeed: (seed) => binding.fetchSeed(seed),
        inspectFile: (inputPath, stage) => binding.inspectFile(inputPath, stage),
        inspectInput: (inputPath, stage, from, challengePolicy) =>
            binding.inspectInput(inputPath, stage, from, challengePolicy),
    };
}
