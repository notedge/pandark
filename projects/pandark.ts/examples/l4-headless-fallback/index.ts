// L4 — Headless 回退：HTTP fetch 失败自动走 headless 浏览器。
//
// 两层防护对抗反爬：
//   1. browserFallback: "on-fetch-failure" — HTTP 失败（timeout/TLS/空 body）自动 try headless
//   2. browserEndpoint                     — 远程 headless worker（Playwright / Puppeteer 服务）
//
// 典型反爬场景：Cloudflare 5s 挑战、JS 渲染壳、Cookie 墙。
// 单纯 HTTP fetch 只能拿半成品 HTML，headless 才是完整 DOM。
//
//   # 启动 headless worker（可选，没有就只用 HTTP）
//   pandark-browser-worker --port 9333 &
//   # 跑 crawl
//   cd l4-headless-fallback && pnpm install
//   pnpm start https://example.com http://127.0.0.1:9333/

import { createPandark } from "@notedge/pandark";

const seed = process.argv[2] ?? "https://example.com";
const browserEndpoint = process.argv[3]; // 可选 headless worker

const pd = createPandark();

const { exitCode, reportJson } = await pd.crawl(seed, {
  depth: 2,
  budget: 5,
  // 关键：HTTP fetch 失败 → 自动 try headless
  browserFallback: "on-fetch-failure",
  challengePolicy: "fallback-http",
  browserEndpoint,
});

const report = JSON.parse(reportJson);
console.log(`🌱 ${report.schema_version}`);
console.log(
  `   extracted=${report.extracted}  failed=${report.failed}  skipped=${report.skipped}`,
);

console.log(`\n📋 diagnostics:`);
for (const d of report.diagnostics ?? []) {
  const fb = d.message?.fallback ?? "(no fallback)";
  const icon = d.code.includes("browser")
    ? "🧭"
    : d.code.includes("challenge")
      ? "🛡️"
      : d.code.includes("fetch")
        ? "📡"
        : "  ";
  console.log(`   ${icon} [${d.severity}] ${d.code} :: ${fb}`);
}

process.exit(exitCode);
