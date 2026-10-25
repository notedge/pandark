// L4 — 挑战策略：处理 Cloudflare / 登录墙 / 同意弹窗等反爬场景。
//
// Pandark 识别 7 种 PageChallengeState:
//   login-required / challenge-required / rate-limited /
//   access-denied / consent-required / unstable-render / unsupported
//
// challengePolicy 决定遇到挑战怎么办：
//   stop            — 停整个 run（默认，保守）
//   pause-for-op    — 暂停等人工介入后续爬
//   use-session     — 只用已登录的外部 session
//   fallback-http   — 无视挑战，回退 HTTP 抓半成品
//   skip            — 跳过这页，继续 frontier
//
//   cd l4-challenge-policy && pnpm install
//   pnpm start https://example.com skip
//   pnpm start https://example.com stop
//   pnpm start https://example.com fallback-http

import { createPandark } from "@notedge/pandark";

const seed = process.argv[2] ?? "https://example.com";
// stop | pause-for-op | use-session | fallback-http | skip
const policy = process.argv[3] ?? "skip";

const pd = createPandark();
console.log(`🎯 challengePolicy: ${policy}`);

const { exitCode, reportJson } = await pd.crawl(seed, {
  depth: 2,
  budget: 5,
  challengePolicy: policy,
});

const report = JSON.parse(reportJson);
console.log(`\n✅ extracted=${report.extracted}  failed=${report.failed}  skipped=${report.skipped}`);

// ---- 诊断里的具体挑战决策 ----
const challengeCodes = [
  "pandark.browser.challenge.login-required",
  "pandark.browser.challenge.required",
  "pandark.browser.challenge.rate-limited",
  "pandark.browser.challenge.access-denied",
  "pandark.browser.challenge.consent-required",
  "pandark.browser.challenge.unstable-render",
  "pandark.browser.challenge.unsupported",
];

console.log(`\n🔍 challenge-related diagnostics:`);
let found = false;
for (const d of report.diagnostics ?? []) {
  if (challengeCodes.includes(d.code)) {
    found = true;
    const fb = d.message?.fallback ?? "(no fallback)";
    console.log(`   🛡️  [${d.severity}] ${d.code} :: ${fb}`);
  }
}
if (!found) console.log("   (无挑战)");

process.exit(exitCode);
