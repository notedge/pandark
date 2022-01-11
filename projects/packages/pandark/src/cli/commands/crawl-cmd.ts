import { writeFile } from "node:fs/promises";

import type { Cli, ParsedOptions } from "@vmz/commander";

import { resolveCheckpointPath, writeCheckpointIfNeeded } from "../checkpoint-path.js";
import { createContext } from "../context.js";
import { ExitCode } from "../exit-codes.js";
import { optionNumber, optionString } from "../options.js";

export function registerCrawlCommand(cli: Cli): void {
    cli.command("crawl", "cli.cmd.crawl")
        .option("--depth <n>", "cli.opt.depth")
        .option("--budget <n>", "cli.opt.budget")
        .option("--browser-fixtures-dir <dir>", "cli.opt.browserFixturesDir")
        .option("--browser-fallback <policy>", "cli.opt.browserFallback")
        .option("--challenge-policy <policy>", "cli.opt.challengePolicy")
        .option("--browser-endpoint <url>", "cli.opt.browserEndpoint")
        .option("--checkpoint-out <file>", "cli.opt.checkpointOut")
        .option("--checkpoint-dir <dir>", "cli.opt.checkpointDir")
        .option("--cache-dir <dir>", "cli.opt.cacheDir")
        .option("--report <file>", "cli.opt.report")
        .option("--json", "cli.opt.json")
        .action((options: ParsedOptions) => runCrawl(options));
}

async function runCrawl(options: ParsedOptions): Promise<number> {
    const seed = options._[0];
    if (!seed || typeof seed !== "string") {
        console.error("crawl requires a seed URL or path");
        return ExitCode.InvalidArgs;
    }

    const ctx = createContext();
    if (!ctx.bindings) {
        console.error("native bindings are not installed for this platform");
        return ExitCode.Internal;
    }

    const maxDepth = optionNumber(options, "depth");
    const maxRequests = optionNumber(options, "budget");
    const browserFixturesDir = optionString(options, "browser-fixtures-dir");
    const browserFallback = optionString(options, "browser-fallback");
    const challengePolicy = optionString(options, "challenge-policy");
    const browserEndpoint = optionString(options, "browser-endpoint");
    const cacheDir = optionString(options, "cache-dir");
    const response = ctx.bindings.crawlFile(
        seed,
        maxDepth,
        maxRequests,
        browserFixturesDir,
        browserFallback,
        challengePolicy,
        browserEndpoint,
        cacheDir,
    );
    const reportPath = optionString(options, "report");
    const checkpointPath = resolveCheckpointPath(
        optionString(options, "checkpoint-out"),
        optionString(options, "checkpoint-dir"),
        seed,
    );
    const writtenCheckpoint = await writeCheckpointIfNeeded(
        checkpointPath,
        response.checkpointJson,
    );

    if (reportPath) {
        await writeFile(reportPath, response.reportJson, "utf8");
    }

    if (options.json) {
        console.log(
            JSON.stringify(
                {
                    exitCode: response.exitCode,
                    committedPages: response.committedPages,
                    checkpoint: response.checkpointJson
                        ? JSON.parse(response.checkpointJson)
                        : null,
                    checkpointPath: writtenCheckpoint ?? null,
                    report: JSON.parse(response.reportJson),
                },
                null,
                2,
            ),
        );
    } else if (!reportPath) {
        console.log(response.reportJson);
    }

    return response.exitCode;
}
