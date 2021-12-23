use pandark_types::{
    apply_challenge_policy, ChallengeOutcome, ChallengePolicy, ExtractBudget, ExtractContext,
    ExtractResult, BrowserSnapshot, PageChallengeState,
};

use crate::browser::snapshot_to_fetch_artifact;
use crate::extract::{Extractor, run_extract};

/// Extraction blocked by challenge policy before IR construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChallengeBlocked {
    /// Detected page challenge state.
    pub state: PageChallengeState,
    /// Applied policy outcome.
    pub outcome: ChallengeOutcome,
}

/// Run extraction from a browser snapshot with challenge handling.
pub fn run_extract_from_snapshot(
    snapshot: &BrowserSnapshot,
    policy: ChallengePolicy,
    extractors: &[&dyn Extractor],
    budget: ExtractBudget,
    ctx: &ExtractContext,
) -> Result<ExtractResult, ChallengeBlocked> {
    let outcome = apply_challenge_policy(snapshot.challenge_state, policy);
    if outcome != ChallengeOutcome::Proceed {
        return Err(ChallengeBlocked {
            state: snapshot.challenge_state,
            outcome,
        });
    }
    let artifact = snapshot_to_fetch_artifact(snapshot);
    Ok(run_extract(&artifact, extractors, budget, ctx))
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::*;
    use crate::HtmlExtractor;
    use pandark_types::ExtractStatus;

    fn snapshot(state: PageChallengeState) -> BrowserSnapshot {
        BrowserSnapshot {
            requested_url: Url::parse("https://example.com/").expect("url"),
            final_url: Url::parse("https://example.com/").expect("url"),
            document_html: "<html><head><title>T</title></head><body>ok</body></html>".into(),
            captured_at_epoch: 1,
            browser_engine: "fixture".into(),
            profile_id: "test".into(),
            challenge_state: state,
        }
    }

    #[test]
    fn challenge_blocks_extraction_when_policy_stops() {
        let html = HtmlExtractor;
        let extractors: [&dyn Extractor; 1] = [&html];
        let blocked = run_extract_from_snapshot(
            &snapshot(PageChallengeState::LoginRequired),
            ChallengePolicy::Stop,
            &extractors,
            ExtractBudget::default(),
            &ExtractContext::default(),
        )
        .unwrap_err();
        assert_eq!(blocked.outcome, ChallengeOutcome::Stop);
    }

    #[test]
    fn normal_snapshot_extracts() {
        let html = HtmlExtractor;
        let extractors: [&dyn Extractor; 1] = [&html];
        let result = run_extract_from_snapshot(
            &snapshot(PageChallengeState::Normal),
            ChallengePolicy::Stop,
            &extractors,
            ExtractBudget::default(),
            &ExtractContext::default(),
        )
        .expect("extract");
        assert_eq!(result.status, ExtractStatus::Complete);
    }
}
