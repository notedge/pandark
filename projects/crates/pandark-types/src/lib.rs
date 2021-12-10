#![deny(missing_debug_implementations)]
#![warn(missing_docs, rustdoc::missing_crate_level_docs)]
#![doc = include_str!("../readme.md")]

mod error;
mod request;
mod result;

pub use crate::error::{CrawlError, Result};
pub use crate::request::{CrawlBudget, CrawlRequest, CrawlSeed, PolitenessProfile};
pub use crate::result::{CrawlPageStatus, CrawlReport, ExtractOutcome, FetchOutcome};

/// Notedown document semantic IR — Pandark extraction hub type.
pub use notedown_ir;
