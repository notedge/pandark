// L2 — 持久化缓存 + 断点续爬：DiskResponseCache + checkpoint。
//
// Pandark 每次 fetch 都写 cache（URL → body bytes + headers）。
// 下次 crawl 从 checkpoint 恢复（resumeCrawlFile），
// 已抓过的 URL 直接从 cache 读，不发 HTTP。
//
// 对大站点抓取中途挂掉特别重要——2 小时的 crawl 不白跑。
//
//   cd l2-cache-resume && pnpm install
//   mkdir -p .cache .checkpoints
//   pnpm start https://example.com .cache .checkpoints

import { createPandark } from "@notedge/pandark";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";

const seed = process.argv[2] ?? "https://example.com";
const cacheDir = process.argv[3] ?? ".cache";
const checkpointDir = process.argv[4] ?? ".checkpoints";

mkdirSync(cacheDir, { recursive: true });
mkdirSync(checkpointDir, { recursive: true });

const checkpointFile = `${checkpointDir}/checkpoint.json`;
const pd = createPandark();

let report;
let checkpointJson: string | undefined;

if (existsSync(checkpointFile)) {
  // ==== RESUME PATH ====
  console.log(`📂 resuming from checkpoint at ${checkpointDir}`);
  const saved = readFileSync(checkpointFile, "utf8");
  const result = await pd.resume(saved, { cacheDir });
  report = JSON.parse(result.reportJson);
  checkpointJson = result.checkpointJson ?? undefined;
} else {
  // ==== FRESH CRAWL PATH ====
  console.log(`🌱 fresh crawl, saving checkpoint to ${checkpointDir}`);
  const result = await pd.crawl(seed, {
    depth: 2,
    budget: 10,
    cacheDir,
  });
  report = JSON.parse(result.reportJson);
  checkpointJson = result.checkpointJson ?? undefined;
}

console.log(
  `✅ extracted=${report.extracted}  failed=${report.failed}  skipped=${report.skipped}  committed=${report.committed}`,
);
console.log(`💾 cache at ${cacheDir}  (re-run uses it, warm start)`);

if (checkpointJson) {
  writeFileSync(checkpointFile, checkpointJson);
  console.log(`📂 checkpoint saved to ${checkpointFile}`);
} else if (existsSync(checkpointFile)) {
  rmSync(checkpointFile);
  console.log(`✅ run complete, checkpoint cleared`);
}
