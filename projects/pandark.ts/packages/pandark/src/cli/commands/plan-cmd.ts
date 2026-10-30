import type { Cli, ParsedOptions } from "@vmz/commander";

import { createContext } from "../context.js";
import { ExitCode } from "../exit-codes.js";
import { optionNumber } from "../options.js";

export function registerPlanCommand(cli: Cli): void {
    cli.command("plan", "cli.cmd.plan")
        .option("--depth <n>", "cli.opt.depth")
        .option("--budget <n>", "cli.opt.budget")
        .option("--json", "cli.opt.json")
        .action((options: ParsedOptions) => runPlan(options));
}

function runPlan(options: ParsedOptions): number {
    const seed = options._[0];
    if (!seed || typeof seed !== "string") {
        console.error("plan requires a seed URL or path");
        return ExitCode.InvalidArgs;
    }

    const ctx = createContext();
    if (!ctx.bindings) {
        console.error("native bindings are not installed for this platform");
        return ExitCode.Internal;
    }

    const maxDepth = optionNumber(options, "depth");
    const maxRequests = optionNumber(options, "budget");
    const response = ctx.bindings.planCrawl(seed, maxDepth, maxRequests);

    if (options.json) {
        console.log(
            JSON.stringify(
                {
                    frontier: response.frontier,
                    report: JSON.parse(response.reportJson),
                },
                null,
                2,
            ),
        );
    } else {
        console.log(`frontier (${response.frontier.length})`);
        for (const url of response.frontier) {
            console.log(`  ${url}`);
        }
    }

    return ExitCode.Success;
}
