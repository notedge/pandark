//! # Example 3 · 发现性爬取: sitemap.xml → Frontier 扩展
//!
//! 场景: 目标站点提供 `sitemap.xml` (或 `sitemap_index.xml`), 我们不想靠内部
//! 链接盲目发现. Pandark 提供 `sitemap::locations()` 解析 + `sitemap_body()`
//! 自动处理 gzip / content-encoding, 把 <loc> 条目作为种子注入 frontier.
//!
//! 关键点:
//! - sitemap_body() 自动处理 gzip / x-gzip content-encoding
//! - locations() 拒绝非 200 / 非同 origin 的 response
//! - 所有诊断通过 DiagnosticEnvelope 返回 (pandark.fetch.body-budget / decode-failed / extract.unsupported-media)
//!
//! ```sh
//! cargo run -p pandark --example sitemap_follow -- https://example.com/sitemap.xml
//! ```

use pandark::{
    HttpTransport, HtmlExtractor, run_crawl_with_options,
    sitemap,
    types::{CrawlRequest, PolitenessProfile, CrawlBudget},
    CrawlOptions,
    fetch::{FetchClient, fetch_url, transport_for_url},
};

fn main() {
    let sitemap_url = std::env::args().nth(1).unwrap_or_else(|| "https://example.com/sitemap.xml".into());
    println!("▶︎ sitemap follow · {sitemap_url}");

    // Step 1: 拉 sitemap artifact 并解析 <loc> 列表
    let sitemap_artifact = fetch_url(&sitemap_url).expect("fetch sitemap");
    let decoded = sitemap::sitemap_body(&sitemap_artifact).expect("decode sitemap body");
    let (is_index, locs) = sitemap::locations(&decoded).expect("parse sitemap XML");
    println!("  is_index = {is_index}  loc_count = {}", locs.len());

    // Step 2: 把 sitemap 里的每个 URL 作为 seed
    let seeds: Vec<String> = locs.into_iter().take(20).collect(); // 限制初始规模
    let mut request = CrawlRequest::from_seeds(&seeds).expect("build request from sitemap");
    request.politeness = PolitenessProfile::Conservative;
    request.budget = CrawlBudget { max_requests: 100, max_depth: 2 };

    let transport = HttpTransport::new();
    let extractor = HtmlExtractor::new();
    let opts = CrawlOptions::default();

    match run_crawl_with_options(request, transport, &[&extractor], opts) {
        Ok(output) => {
            let r = &output.report;
            println!(
                "✓ done · extracted={} failed={} skipped={}  (from {} sitemap URLs)",
                r.extracted, r.failed, r.skipped, r.events.discovered.len(),
            );
        }
        Err(e) => {
            eprintln!("✗ crawl failed: {e}");
            std::process::exit(1);
        }
    }
}
