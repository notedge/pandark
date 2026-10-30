import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";

/** Build a stable checkpoint filename from a crawl seed URL or path. */
export function checkpointSlug(seed: string): string {
    const normalized = seed
        .replace(/^https?:\/\//, "")
        .replace(/[^a-zA-Z0-9]+/g, "-")
        .replace(/^-+|-+$/g, "")
        .slice(0, 48);
    return normalized || "seed";
}

/** Resolve where a paused crawl checkpoint should be written. */
export function resolveCheckpointPath(
    checkpointOut: string | undefined,
    checkpointDir: string | undefined,
    seed: string,
): string | undefined {
    if (checkpointOut) {
        return checkpointOut;
    }
    if (!checkpointDir) {
        return undefined;
    }
    const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
    return join(checkpointDir, `pandark-checkpoint-${checkpointSlug(seed)}-${timestamp}.json`);
}

/** Write checkpoint JSON when a crawl paused and a target path is configured. */
export async function writeCheckpointIfNeeded(
    checkpointPath: string | undefined,
    checkpointJson: string | undefined,
): Promise<string | undefined> {
    if (!checkpointPath || !checkpointJson) {
        return undefined;
    }
    await mkdir(dirname(checkpointPath), { recursive: true });
    await writeFile(checkpointPath, checkpointJson, "utf8");
    return checkpointPath;
}
