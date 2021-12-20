import type { Cli, ParsedOptions } from "@vmz/commander";

import { createContext } from "../context.js";
import { ExitCode } from "../exit-codes.js";

type DoctorCheck = {
    id: string;
    ok: boolean;
    message: string;
};

export function registerDoctorCommand(cli: Cli): void {
    cli.command("doctor", "cli.cmd.doctor")
        .option("--json", "cli.opt.json")
        .action((options: ParsedOptions) => runDoctor(options));
}

function runDoctor(options: ParsedOptions): number {
    const checks: DoctorCheck[] = [];
    checks.push({
        id: "node",
        ok: Number(process.versions.node.split(".")[0]) >= 20,
        message: `node ${process.version}`,
    });

    const ctx = createContext();
    if (ctx.bindings) {
        checks.push({
            id: "native",
            ok: true,
            message: `pandark-napi ${ctx.bindings.pandarkVersion()}`,
        });
    } else {
        checks.push({
            id: "native",
            ok: false,
            message: "native bindings are not installed for this platform",
        });
    }

    const ok = checks.every((check) => check.ok);
    if (options.json) {
        console.log(JSON.stringify({ ok, checks }, null, 2));
    } else {
        for (const check of checks) {
            console.log(`${check.ok ? "ok" : "fail"} ${check.id}: ${check.message}`);
        }
    }

    return ok ? ExitCode.Success : ExitCode.Internal;
}
