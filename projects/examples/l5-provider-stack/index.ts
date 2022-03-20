// L5 — 多层 Provider Stack: Fixture → Headless → HTTP 兜底的完整对抗链。
//
// Pandark BrowserProviderStack 按顺序尝试：
//   1. FixtureBrowserProvider — 离线 snapshot（零网络，最快）
//   2. HttpBrowserProvider    — 远程 headless worker（JS 渲染 + 登录态）
//   3. HTTP fetch             — 兜底（可能抓半成品）
//
// 对抗 Cloudflare / 复杂 SPA / 需要登录的站点的完整方案。
// 中间任何一层成功就不再向下 fallback。
//
//   cd l5-provider-stack && pnpm install
//   pnpm start https://example.com fixtures http://127.0.0.1:9333/
//   # 没有 headless worker 也能跑（最后 HTTP 兜底）
//   pnpm start https://example.com fixtures

import { createPandark } from "@notedge/pandark";

const seed = process.argv[2] ?? "https://example.com";
const fixtureDir = process.argv[3] ?? "fixtures";
const headlessEndpoint = process.argv[4]; // 可选

const pd = createPandark();

console.log(`🏗️  browser stack built:`);
console.log(`   fixture dir: ${fixtureDir}`);
console.log(`   headless worker: ${headlessEndpoint ?? "(none — HTTP-only)"}`);
console.log(`   seed: ${seed}`);

const { exitCode, reportJson } = pd.crawl(seed, {
  depth: 2,
  budget: 10,
  // ==== provider stack ====
  browserFixturesDir: fixtureDir,
  browserFallback: "on-fetch-failure",
  browserEndpoint: headlessEndpoint,
  // ==== 挑战策略: 遇 Cloudflare 跳过继续 ====
  challengePolicy: "skip",
});

const report = JSON.parse(reportJson);
console.log(`\n✅ ${report.schema_version}`);
console.log(
  `   extracted=${report.extracted}  failed=${report.failed}  skipped=${report.skipped}  committed=${report.committed}`,
);

console.log(`\n📋 diagnostics (${report.diagnostics?.length ?? 0}):`);
for (const d of report.diagnostics ?? []) {
  const fb = d.message?.fallback ?? "(no fallback)";
  const icon = d.code.includes("browser")
    ? "🧭"
    : d.code.includes("challenge")
      ? "🛡️"
      : d.code.includes("fetch")
        ? "📡"
        : d.code.includes("robots")
          ? "🤖"
          : d.code.includes("politeness")
            ? "⏱️"
            : "  ";
  console.log(`   ${icon} [${d.severity}] ${d.code} :: ${fb}`);
}

process.exit(exitCode);
