// L1 — 反限流：指数退避 ±25% jitter + 自动重试。
//
// Pandark fetch client 内置：
//   base = 2^min(attempt, 4) 秒，最大 30s，叠加 ±25% 随机抖动
//
//   429 / 5xx  → 自动重试（Retryable）
//   404 / 410  → 永久跳过（PermanentFailure）
//   401 / 403  → 直接失败（ClientError）
//   其他       → Proceed 继续
//
// Conservative profile 开 robots + 低并发；Balanced 放宽；Developer 几乎无限制。
//
//   cd l1-polite-backoff && pnpm install && pnpm start https://httpbin.org/delay/1

import { createPandark } from "@notedge/pandark";

const seed = process.argv[2] ?? "https://example.com";
const pd = createPandark();

const { exitCode, reportJson } = await pd.crawl(seed, {
  depth: 2,
  budget: 5,
});

const report = JSON.parse(reportJson);
console.log(`🌱 ${report.schema_version}`);
console.log(
  `   extracted=${report.extracted}  failed=${report.failed}  skipped=${report.skipped}`,
);

// 诊断里能看到退避相关的结构化 code：
//   pandark.politeness.rate-limited   — 429 触发
//   pandark.politeness.crawl-delay    — robots crawl-delay
//   pandark.fetch.timeout             — 超时
//   pandark.fetch.http-status         — 非 2xx
console.log(`\n📋 diagnostics breakdown:`);
for (const d of report.diagnostics ?? []) {
  const code = d.code as string;
  const fb = d.message?.fallback ?? "(no fallback)";
  const icon = code.includes("robots")
    ? "🤖"
    : code.includes("politeness")
      ? "⏱️"
      : code.includes("fetch.http")
        ? "📡"
        : "  ";
  console.log(`   ${icon} [${d.severity}] ${code} :: ${fb}`);
}

process.exit(exitCode);
