import { writeFile } from "node:fs/promises";

import type { Cli, ParsedOptions } from "@vmz/commander";

import { createContext } from "../context.js";
import { ExitCode } from "../exit-codes.js";

export function registerExtractCommand(cli: Cli): void {
    cli.command("extract", "cli.cmd.extract")
        .option("-o, --output <file>", "cli.opt.output")
        .option("--report <file>", "cli.opt.report")
        .option("--json", "cli.opt.json")
        .action((options: ParsedOptions) => runExtract(options));
}

async function runExtract(options: ParsedOptions): Promise<number> {
    const inputPath = options._[0];
    if (!inputPath || typeof inputPath !== "string") {
        console.error("extract requires an input file path");
        return ExitCode.InvalidArgs;
    }

    const ctx = createContext();
    if (!ctx.bindings) {
        console.error("native bindings are not installed for this platform");
        return ExitCode.Internal;
    }

    const response = ctx.bindings.extractFile(inputPath);
    const outputPath = typeof options.output === "string" ? options.output : undefined;
    const reportPath = typeof options.report === "string" ? options.report : undefined;

    if (outputPath && response.documentJson) {
        await writeFile(outputPath, response.documentJson, "utf8");
    }
    if (reportPath) {
        await writeFile(reportPath, response.reportJson, "utf8");
    }

    if (options.json) {
        console.log(
            JSON.stringify(
                {
                    exitCode: response.exitCode,
                    status: response.status,
                    document: response.documentJson ? JSON.parse(response.documentJson) : null,
                    report: JSON.parse(response.reportJson),
                },
                null,
                2,
            ),
        );
    } else if (!outputPath) {
        console.log(response.reportJson);
    }

    return response.exitCode;
}
