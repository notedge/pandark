import { writeFile } from "node:fs/promises";

import type { Cli, ParsedOptions } from "@vmz/commander";

import { createContext } from "../context.js";
import { ExitCode } from "../exit-codes.js";

export function registerFetchCommand(cli: Cli): void {
    cli.command("fetch", "cli.cmd.fetch")
        .option("-o, --output <file>", "cli.opt.output")
        .option("--json", "cli.opt.json")
        .action((options: ParsedOptions) => runFetch(options));
}

async function runFetch(options: ParsedOptions): Promise<number> {
    const seed = options._[0];
    if (!seed || typeof seed !== "string") {
        console.error("fetch requires a URL or path");
        return ExitCode.InvalidArgs;
    }

    const ctx = createContext();
    if (!ctx.bindings) {
        console.error("native bindings are not installed for this platform");
        return ExitCode.Internal;
    }

    const response = ctx.bindings.fetchSeed(seed);
    const outputPath = typeof options.output === "string" ? options.output : undefined;

    if (outputPath) {
        await writeFile(outputPath, response.artifactJson, "utf8");
    }

    if (options.json) {
        console.log(
            JSON.stringify(
                {
                    exitCode: response.exitCode,
                    artifact: JSON.parse(response.artifactJson),
                },
                null,
                2,
            ),
        );
    } else if (!outputPath) {
        console.log(response.artifactJson);
    }

    return response.exitCode;
}
