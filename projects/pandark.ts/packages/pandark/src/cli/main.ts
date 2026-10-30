import { buildPandarkCli } from "./cli.js";

export async function runCli(argv: string[]): Promise<number> {
    try {
        return await buildPandarkCli().parse(argv);
    } catch (error) {
        console.error(`error: ${error instanceof Error ? error.message : String(error)}`);
        return 2;
    }
}
