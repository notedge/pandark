// L0 — 零防护情景：一条 seed，默认 HTTP 传输。
//
// 最简单的起点：网络可达 → HTML 到手 → extractor 产出 IR。
// 没有退避、没有 robots、没有 headless、没有缓存。
// 适合一次性抓静态文档站点。
//
//   cd l0-hello-fetch && pnpm install && pnpm start https://example.com

import { createPandark } from "@notedge/pandark";

const seed = process.argv[2] ?? "https://example.com";
const pd = createPandark();
console.log(`pandark ${pd.version()}`);

const { exitCode, reportJson } = pd.crawl(seed);
if (exitCode !== 0) {
  console.error(`crawl failed: exit ${exitCode}`);
  console.error(reportJson);
  process.exit(exitCode);
}

const report = JSON.parse(reportJson);
const { schema_version, seed: seedUrl, extracted, failed, skipped, diagnostics } = report;

console.log(`\n🌱 ${schema_version}`);
console.log(`   seed=${seedUrl}`);
console.log(`   extracted=${extracted}  failed=${failed}  skipped=${skipped}`);
console.log(`   diagnostics: ${diagnostics?.length ?? 0}`);

if (diagnostics?.length) {
  for (const d of diagnostics) {
    const fb = d.message?.fallback ?? "(no fallback)";
    console.log(`   [${d.severity}] ${d.code} :: ${fb}`);
  }
}
