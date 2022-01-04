import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { createCli } from "@vmz/commander";

import { registerPlanCommand } from "./commands/plan-cmd.js";
import { registerCrawlCommand } from "./commands/crawl-cmd.js";
import { registerDoctorCommand } from "./commands/doctor-cmd.js";
import { registerExtractCommand } from "./commands/extract-cmd.js";
import { registerFetchCommand } from "./commands/fetch-cmd.js";
import { registerInspectCommand } from "./commands/inspect-cmd.js";
import { registerResumeCommand } from "./commands/resume-cmd.js";

export function buildPandarkCli() {
    const localesRoot = join(dirname(fileURLToPath(import.meta.url)), "../../locales");
    const cli = createCli("pandark")
        .locales(localesRoot, { envKeys: ["PANDARK_LOCALE", "LOCALE", "LANG", "LC_ALL"] })
        .intro("cli.intro");

    registerPlanCommand(cli);
    registerFetchCommand(cli);
    registerCrawlCommand(cli);
    registerResumeCommand(cli);
    registerExtractCommand(cli);
    registerInspectCommand(cli);
    registerDoctorCommand(cli);

    return cli;
}
