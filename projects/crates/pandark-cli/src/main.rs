//! Pandark CLI — thin command-line wrapper over `pandark` library.
//!
//! Commands directly call the published crawl / extract / fetch / inspect
//! functions. No NAPI bridge, no new scheduling layers.

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use pandark::CrawlOptions;
use pandark::fetch::HttpTransport;
use pandark::semantic::HtmlExtractor;
use pandark::types::CrawlRequest;
use pandark::{plan_crawl, run_crawl_with_options};

/// Pandark — web crawl + extract CLI.
#[derive(Debug, Parser)]
#[command(name = "pandark", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show Pandark version and configured transports.
    Version,

    /// Build a crawl plan from seed URLs without making network I/O.
    Plan {
        /// One or more seed URLs (repeatable).
        #[arg(required = true, num_args = 1..)]
        seeds: Vec<String>,
        /// Maximum link depth from seeds.
        #[arg(long = "max-depth", default_value_t = 2)]
        max_depth: u32,
        /// Maximum HTTP requests including assets.
        #[arg(long = "max-requests", default_value_t = 500)]
        max_requests: u32,
    },

    /// Crawl one or more seed URLs up to the configured depth.
    Crawl {
        /// One or more seed URLs (repeatable).
        #[arg(required = true, num_args = 1..)]
        seeds: Vec<String>,
        /// Maximum link depth from seeds.
        #[arg(long = "max-depth", default_value_t = 2)]
        max_depth: u32,
        /// Maximum HTTP requests including assets.
        #[arg(long = "max-requests", default_value_t = 500)]
        max_requests: u32,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("pandark: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Version => cmd_version(),
        Command::Plan { seeds, max_depth, max_requests } => cmd_plan(&seeds, max_depth, max_requests),
        Command::Crawl { seeds, max_depth, max_requests } => cmd_crawl(&seeds, max_depth, max_requests),
    }
}

fn cmd_version() -> Result<(), String> {
    println!("pandark {}", env!("CARGO_PKG_VERSION"));
    println!("transport:  http");
    println!("extractors: html");
    println!("note:       crawl command requires network access");
    Ok(())
}

fn cmd_plan(seeds: &[String], max_depth: u32, max_requests: u32) -> Result<(), String> {
    let request = build_request(seeds, max_depth, max_requests)?;
    let plan = plan_crawl(request).map_err(|e| e.to_string())?;
    println!("seeds:    {}", plan.request.seeds.len());
    println!("frontier: {} item(s)", plan.frontier.len());
    for url in &plan.frontier {
        println!("  {url}");
    }
    Ok(())
}

fn cmd_crawl(seeds: &[String], max_depth: u32, max_requests: u32) -> Result<(), String> {
    let request = build_request(seeds, max_depth, max_requests)?;
    let transport = HttpTransport::default();
    let extractor = HtmlExtractor;
    let options = CrawlOptions::default();
    let extractors: Vec<&dyn pandark::Extractor> = vec![&extractor];
    let output = run_crawl_with_options(request, transport, &extractors, options).map_err(|e| e.to_string())?;

    println!("crawl complete");
    println!("  pages committed: {}", output.committed_pages.len());
    println!("  artifacts:       {}", output.artifacts.len());
    if output.checkpoint.is_some() {
        println!("  status:          paused for operator action");
    }
    Ok(())
}

fn build_request(seeds: &[String], max_depth: u32, max_requests: u32) -> Result<CrawlRequest, String> {
    let mut request = CrawlRequest::from_seeds(seeds).map_err(|e| e.to_string())?;
    request.budget.max_depth = max_depth;
    request.budget.max_requests = max_requests;
    Ok(request)
}
