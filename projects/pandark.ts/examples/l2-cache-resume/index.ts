// L2 — 持久化 store + 断点续爬：`.yydx` store。
//
// Pandark 把 frontier、页面事务、对象与报告事件都写进 store，
// 暂停后用运行 id 从同一个 store 恢复，不再需要 JSON checkpoint。
//
// 对大站点抓取中途挂掉特别重要——2 小时的 crawl 不白跑。
//
//   cd l2-cache-resume && pnpm install
//   pnpm start https://example.com .pandark/run.yydx

import { createPandark } from "@notedge/pandark";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

const seed = process.argv[2] ?? "https://example.com";
const storePath = process.argv[3] ?? ".pandark/run.yydx";

mkdirSync(dirname(storePath), { recursive: true });

const runIdFile = `${storePath}.run-id`;
const pd = createPandark();

let report;
let runId: string | undefined;
let paused = false;

if (existsSync(runIdFile)) {
    // ==== RESUME PATH ====
    const savedRunId = readFileSync(runIdFile, "utf8").trim();
    console.log(`📂 resuming run ${savedRunId} from ${storePath}`);
    const result = await pd.resume(savedRunId, { storePath });
    report = JSON.parse(result.reportJson);
    runId = result.runId ?? savedRunId;
    paused = (report.paused ?? 0) > 0;
} else {
    // ==== FRESH CRAWL PATH ====
    console.log(`🌱 fresh crawl into ${storePath}`);
    const result = await pd.crawl(seed, {
        depth: 2,
        budget: 10,
        storePath,
    });
    report = JSON.parse(result.reportJson);
    runId = result.runId;
    paused = result.exitCode === 3;
}

console.log(
    `✅ extracted=${report.extracted}  failed=${report.failed}  skipped=${report.skipped}  committed=${report.committed}`,
);
console.log(`💾 store at ${storePath}  (re-run resumes it)`);

if (paused && runId) {
    writeFileSync(runIdFile, runId);
    console.log(`📂 run id saved to ${runIdFile}`);
} else if (existsSync(runIdFile)) {
    rmSync(runIdFile);
    console.log(`✅ run complete, run id cleared`);
}
