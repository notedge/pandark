//! # Example 2 · 反爬第一步: 指数退避 + Conservative Politeness
//!
//! 场景: 小型站点但 host 有速率限制 (HTTP 429 / 503). Pandark 内置指数退避
//! + 25% jitter, 配合 Conservative politeness preset 把请求频率压到每 host
//! 大约 1 req/3s, 足以覆盖绝大多数轻度限流.
//!
//! 关键点:
//! - `PolitenessProfile::Conservative` 把 min_delay_ms 抬到 3000
//! - Pandark-fetch 已内置 `classify_http_status()` 把 429/500/502/503 标为 Retryable
//! - HttpTransport 自动对 Retryable 状态执行指数退避重试
//! - PermanentFailure (404/410) / ClientError (401/403) 直接 skip
//!
//! ```sh
//! cargo run -p pandark --example polite -- https://news.ycombinator.com
//! ```

use pandark::{
    HttpTransport, HtmlExtractor, run_crawl_with_options,
    types::{CrawlRequest, PolitenessProfile, CrawlBudget},
    CrawlOptions,
};

fn main() {
    let seed = std::env::args().nth(1).unwrap_or_else(|| "https://news.ycombinator.com".into());
    println!("▶︎ polite crawl · seed = {seed}");
    println!("  politeness = Conservative  (min_delay 3s)");
    println!("  retries    = built-in classify_http_status (429/5xx → Retryable)");
    println!("  budget     = 50 requests, depth 2");

    let mut request = CrawlRequest::from_seed(&seed).expect("valid seed URL");
    request.politeness = PolitenessProfile::Conservative;
    request.budget = CrawlBudget { max_requests: 50, max_depth: 2 };

    let transport = HttpTransport::new();
    let extractor = HtmlExtractor::new();

    let opts = CrawlOptions::default();

    match run_crawl_with_options(request, transport, &[&extractor], opts) {
        Ok(output) => {
            let r = &output.report;
            println!(
                "✓ done · extracted={} failed={} skipped={}",
                r.extracted, r.failed, r.skipped,
            );
        }
        Err(e) => {
            eprintln!("✗ crawl failed: {e}");
            std::process::exit(1);
        }
    }
}
