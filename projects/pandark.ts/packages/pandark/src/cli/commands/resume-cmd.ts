import { writeFile } from "node:fs/promises";

import type { Cli, ParsedOptions } from "@vmz/commander";

import { createContext } from "../context.js";
import { ExitCode } from "../exit-codes.js";
import { optionString } from "../options.js";

export function registerResumeCommand(cli: Cli): void {
    cli.command("resume", "cli.cmd.resume")
        .option("--browser-fixtures-dir <dir>", "cli.opt.browserFixturesDir")
        .option("--browser-fallback <policy>", "cli.opt.browserFallback")
        .option("--challenge-policy <policy>", "cli.opt.challengePolicy")
        .option("--browser-endpoint <url>", "cli.opt.browserEndpoint")
        .option("--store <file>", "cli.opt.store")
        .option("--report <file>", "cli.opt.report")
        .option("--json", "cli.opt.json")
        .action((options: ParsedOptions) => runResume(options));
}

async function runResume(options: ParsedOptions): Promise<number> {
    const runId = options._[0];
    if (!runId || typeof runId !== "string") {
        console.error("resume requires a run id from a store-backed crawl");
        return ExitCode.InvalidArgs;
    }

    const ctx = createContext();
    if (!ctx.bindings) {
        console.error("native bindings are not installed for this platform");
        return ExitCode.Internal;
    }

    const storePath = optionString(options, "store");
    if (!storePath) {
        console.error("resume requires --store <file.yydx>");
        return ExitCode.InvalidArgs;
    }

    const browserFixturesDir = optionString(options, "browser-fixtures-dir");
    const browserFallback = optionString(options, "browser-fallback");
    const challengePolicy = optionString(options, "challenge-policy");
    const browserEndpoint = optionString(options, "browser-endpoint");
    const response = await ctx.bindings.resumeCrawlFile(
        runId,
        browserFixturesDir,
        browserFallback,
        challengePolicy,
        browserEndpoint,
        storePath,
    );

    const reportPath = optionString(options, "report");

    if (reportPath) {
        await writeFile(reportPath, response.reportJson, "utf8");
    }

    if (options.json) {
        console.log(
            JSON.stringify(
                {
                    exitCode: response.exitCode,
                    committedPages: response.committedPages,
                    runId: response.runId ?? runId,
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
