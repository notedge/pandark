//! # Example 4 · 规模化爬取: DiskResponseCache + checkpoint resume
//!
//! 场景: 大站点爬取被预算截断 / 进程被杀掉, 下次重启希望从上次 frontier
//! 继续, 并且不想对已爬 URL 重新发 HTTP 请求.
//!
//! 关键点:
//! - `FetchClient::with_disk_cache_dir()` 把 FetchClient 接到磁盘缓存
//! - `run_crawl_with_options(cache_dir = Some(...))` 让 crawl 本身也落盘 checkpoint
//! - `resume_crawl_from_checkpoint()` 从 checkpoint.json 恢复 frontier / report
//! - 幂等: 同样策略指纹 + attempt_identity = 0 表示第一次跑, > 0 表示恢复
//!
//! ```sh
//! mkdir -p ./pandark-cache
//! cargo run -p pandark --example cache_resume -- https://example.com ./pandark-cache
//! # kill -9 it midway, then run again — it resumes from last checkpoint
//! ```

use std::path::PathBuf;

use pandark::{
    CrawlOptions, HttpTransport, HtmlExtractor, run_crawl_with_options, resume_crawl_from_checkpoint,
    types::{CrawlRequest, PolitenessProfile, CrawlBudget},
};

fn main() {
    let mut args = std::env::args().skip(1);
    let seed = args.next().unwrap_or_else(|| "https://example.com".into());
    let cache_dir: PathBuf = args.next().unwrap_or_else(|| "./pandark-cache".into()).into();

    println!("▶︎ cache + resume crawl · seed = {seed}");
    println!("  cache_dir = {}", cache_dir.display());

    let mut request = CrawlRequest::from_seed(&seed).expect("valid seed URL");
    request.politeness = PolitenessProfile::Conservative;
    request.budget = CrawlBudget { max_requests: 200, max_depth: 3 };

    let transport = HttpTransport::new();
    let extractor = HtmlExtractor::new();

    let checkpoint_path = cache_dir.join("checkpoint.json");

    let result = if checkpoint_path.exists() {
        println!("  ↳ resuming from existing checkpoint");
        let resume = resume_crawl_from_checkpoint(&checkpoint_path).expect("read checkpoint");
        let mut opts = CrawlOptions::default();
        opts.cache_dir = Some(&cache_dir);
        opts.strategy_fingerprint = Some("pandark-examples/cache_resume/v1");
        opts.attempt_identity = resume.attempt_identity + 1;
        run_crawl_with_options(resume.request, transport, &[&extractor], opts)
    }
    else {
        println!("  ↳ fresh run (no checkpoint yet)");
        let mut opts = CrawlOptions::default();
        opts.cache_dir = Some(&cache_dir);
        opts.strategy_fingerprint = Some("pandark-examples/cache_resume/v1");
        opts.attempt_identity = 0;
        run_crawl_with_options(request, transport, &[&extractor], opts)
    };

    match result {
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
