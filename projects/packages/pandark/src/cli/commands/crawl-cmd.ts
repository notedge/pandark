import { writeFile } from "node:fs/promises";

import type { Cli, ParsedOptions } from "@vmz/commander";

import { createContext } from "../context.js";
import { ExitCode } from "../exit-codes.js";

export function registerCrawlCommand(cli: Cli): void {
    cli.command("crawl", "cli.cmd.crawl")
        .option("--depth <n>", "cli.opt.depth")
        .option("--budget <n>", "cli.opt.budget")
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

    const maxDepth = readOptionalNumber(options.depth);
    const maxRequests = readOptionalNumber(options.budget);
    const response = ctx.bindings.crawlFile(seed, maxDepth, maxRequests);
    const reportPath = typeof options.report === "string" ? options.report : undefined;

    if (reportPath) {
        await writeFile(reportPath, response.reportJson, "utf8");
    }

    if (options.json) {
        console.log(
            JSON.stringify(
                {
                    exitCode: response.exitCode,
                    committedPages: response.committedPages,
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

function readOptionalNumber(value: unknown): number | undefined {
    if (value === undefined || value === null || value === "") {
        return undefined;
    }
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : undefined;
}
