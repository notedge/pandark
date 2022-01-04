import { readFile, writeFile } from "node:fs/promises";

import type { Cli, ParsedOptions } from "@vmz/commander";

import { createContext } from "../context.js";
import { ExitCode } from "../exit-codes.js";
import { optionString } from "../options.js";

export function registerResumeCommand(cli: Cli): void {
    cli.command("resume", "cli.cmd.resume")
        .option("--browser-fixtures-dir <dir>", "cli.opt.browserFixturesDir")
        .option("--browser-fallback <policy>", "cli.opt.browserFallback")
        .option("--challenge-policy <policy>", "cli.opt.challengePolicy")
        .option("--checkpoint-out <file>", "cli.opt.checkpointOut")
        .option("--report <file>", "cli.opt.report")
        .option("--json", "cli.opt.json")
        .action((options: ParsedOptions) => runResume(options));
}

async function runResume(options: ParsedOptions): Promise<number> {
    const checkpointPath = options._[0];
    if (!checkpointPath || typeof checkpointPath !== "string") {
        console.error("resume requires a checkpoint JSON file path");
        return ExitCode.InvalidArgs;
    }

    const ctx = createContext();
    if (!ctx.bindings) {
        console.error("native bindings are not installed for this platform");
        return ExitCode.Internal;
    }

    const checkpointJson = await readFile(checkpointPath, "utf8");
    const browserFixturesDir = optionString(options, "browser-fixtures-dir");
    const browserFallback = optionString(options, "browser-fallback");
    const challengePolicy = optionString(options, "challenge-policy");
    const response = ctx.bindings.resumeCrawlFile(
        checkpointJson,
        browserFixturesDir,
        browserFallback,
        challengePolicy,
    );

    const reportPath = optionString(options, "report");
    const checkpointOut = optionString(options, "checkpoint-out");

    if (reportPath) {
        await writeFile(reportPath, response.reportJson, "utf8");
    }
    if (checkpointOut && response.checkpointJson) {
        await writeFile(checkpointOut, response.checkpointJson, "utf8");
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
