// L4 — Fixture 回放：离线 snapshot 驱动，CI 无网络也能跑。
//
// FixtureBrowserProvider 把预先录好的浏览器 snapshot (JSON)
// 当作"真实"浏览器响应喂给 extractor。
//
// 典型场景：
//   - CI 测试不想连外网
//   - 反爬站点 snapshot 一次录好，后续重复提取
//   - 给下游 SDK 发 fixture 做集成测试
//
// fixture 目录结构（pandark 自动扫描）：
//   fixtures/
//     └── fixture.json   # 单个 snapshot（带 url / html / final_url / status）
//
//   cd l4-fixture-replay && pnpm install
//   pnpm start fixtures https://example.com

import { createPandark } from "@notedge/pandark";
import { existsSync } from "node:fs";

const fixtureDir = process.argv[2] ?? "fixtures";
const seed = process.argv[3] ?? "https://example.com";

const pd = createPandark();
console.log(`📂 browserFixturesDir: ${fixtureDir}`);
console.log(`🌱 seed: ${seed}`);

const { exitCode, reportJson } = pd.crawl(seed, {
  depth: 0,
  budget: 1,
  browserFixturesDir: fixtureDir,
  browserFallback: "on-fetch-failure",
});

const report = JSON.parse(reportJson);
console.log(
  `\n✅ extracted=${report.extracted}  failed=${report.failed}  skipped=${report.skipped}`,
);

// ---- fixture 提示 ----
if (!existsSync(fixtureDir)) {
  console.log(`\n💡 提示：fixture 目录不存在。录一个试试：`);
  console.log(`   pandark fetch ${seed} --out fixtures/fixture.json`);
}

process.exit(exitCode);
