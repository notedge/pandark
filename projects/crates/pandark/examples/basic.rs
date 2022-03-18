//! # Example 1 · 最简 Hello World Crawl
//!
//! 场景: 一个公开可访问的 seed 页面, 不要求发现性, 零配置启动.
//! 这是 Pandark 入口: `run_crawl` 接受 `CrawlRequest + Transport + Extractor`.
//!
//! ```sh
//! cargo run -p pandark --example basic -- https://example.com
//! ```

use pandark::{
    HttpTransport, HtmlExtractor, run_crawl,
    types::{CrawlRequest, PolitenessProfile},
};

fn main() {
    let seed = std::env::args().nth(1).unwrap_or_else(|| "https://example.com".into());
    println!("▶︎ basic crawl · seed = {seed}");

    let request = CrawlRequest::from_seed(&seed).expect("valid seed URL");
    let transport = HttpTransport::new();
    let extractor = HtmlExtractor::new();

    match run_crawl(request, transport, &[&extractor]) {
        Ok(output) => {
            let r = &output.report;
            println!(
                "✓ done · extracted={} failed={} skipped={}",
                r.extracted, r.failed, r.skipped,
            );
            for diag in &r.diagnostics.diagnostics {
                println!("  ⚠︎ {} :: {}", diag.code(), diag.fallback().unwrap_or_default());
            }
        }
        Err(e) => {
            eprintln!("✗ crawl failed: {e}");
            std::process::exit(1);
        }
    }
}
