import type { Cli, ParsedOptions } from "@vmz/commander";

import { createContext } from "../context.js";
import { ExitCode } from "../exit-codes.js";

export function registerInspectCommand(cli: Cli): void {
    cli.command("inspect", "cli.cmd.inspect")
        .option("--stage <stage>", "cli.opt.stage")
        .option("--json", "cli.opt.json")
        .action((options: ParsedOptions) => runInspect(options));
}

function runInspect(options: ParsedOptions): number {
    const inputPath = options._[0];
    if (!inputPath || typeof inputPath !== "string") {
        console.error("inspect requires an input file path");
        return ExitCode.InvalidArgs;
    }

    const ctx = createContext();
    if (!ctx.bindings) {
        console.error("native bindings are not installed for this platform");
        return ExitCode.Internal;
    }

    const stage = typeof options.stage === "string" ? options.stage : undefined;
    const response = ctx.bindings.inspectFile(inputPath, stage);

    if (options.json) {
        console.log(
            JSON.stringify(
                {
                    exitCode: response.exitCode,
                    report: JSON.parse(response.reportJson),
                },
                null,
                2,
            ),
        );
    } else {
        console.log(response.reportJson);
    }

    return response.exitCode;
}
