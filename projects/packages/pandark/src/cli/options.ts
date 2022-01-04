import type { ParsedOptions } from "@vmz/commander";

/** Read a string CLI option registered with `@vmz/commander` kebab-case keys. */
export function optionString(options: ParsedOptions, key: string): string | undefined {
    const value = options[key];
    return typeof value === "string" ? value : undefined;
}

/** Read a numeric CLI option when present and finite. */
export function optionNumber(options: ParsedOptions, key: string): number | undefined {
    const value = optionString(options, key);
    if (value === undefined) {
        return undefined;
    }
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : undefined;
}
