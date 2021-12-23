#![deny(missing_debug_implementations)]
#![warn(missing_docs, rustdoc::missing_crate_level_docs)]
#![doc = include_str!("../readme.md")]

mod artifact;
mod browser;
mod challenge;
mod error;
mod events;
mod extract;
mod frontier;
mod inspect;
mod request;
mod result;
mod url;

pub use crate::artifact::{
    BodyStorage, FetchArtifact, HttpStatus, RedirectHop, TransportProvenance,
};
pub use crate::browser::{
    AuthProfile, BrowserActionKind, BrowserSnapshot, ChallengePolicy, CredentialRef,
    PageChallengeState, SessionPersistence,
};
pub use crate::challenge::{ChallengeOutcome, apply_challenge_policy};
pub use crate::error::{CrawlError, Result};
pub use crate::events::{CrawlEvent, EventLog, PageTransaction};
pub use crate::extract::{
    ExtractBudget, ExtractContext, ExtractInput, ExtractOutcome, ExtractResult, ExtractStatus,
    ProbeResult, ProbeStatus,
};
pub use crate::frontier::{
    AdmissionDecision, DiscoverySource, FrontierItem, LinkCandidate, LinkRelation,
};
pub use crate::inspect::{InspectReport, InspectStage};
pub use crate::request::{
    BrowserFallbackPolicy, CrawlBudget, CrawlRequest, CrawlSeed, PolitenessProfile,
};
pub use crate::result::{CrawlPageStatus, CrawlReport};
pub use crate::url::{canonicalize_parsed_url, canonicalize_request_url, now_epoch, site_key};

/// Notedown document semantic IR — Pandark extraction hub type.
pub use notedown_ir;
