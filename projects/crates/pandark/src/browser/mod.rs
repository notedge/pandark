use std::collections::BTreeMap;

use pandark_types::{
    BodyStorage, BrowserSnapshot, FetchArtifact, PageChallengeState, TransportProvenance,
};

/// Convert a browser snapshot into a fetch artifact for the existing extract pipeline.
pub fn snapshot_to_fetch_artifact(snapshot: &BrowserSnapshot) -> FetchArtifact {
    let body = BodyStorage::inline(snapshot.document_html.as_bytes().to_vec());
    let headers = BTreeMap::from([("content-type".into(), "text/html".into())]);
    FetchArtifact {
        request_url: snapshot.requested_url.clone(),
        final_url: snapshot.final_url.clone(),
        status: if snapshot.challenge_state == PageChallengeState::Normal {
            200
        } else {
            503
        },
        response_headers: headers,
        content_type: Some("text/html".into()),
        body,
        redirect_chain: Vec::new(),
        retrieved_at_epoch: snapshot.captured_at_epoch,
        transport_provenance: TransportProvenance::Test,
        cache_key: format!("browser:{}:{}", snapshot.profile_id, snapshot.final_url),
    }
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::*;

    #[test]
    fn snapshot_produces_html_artifact() {
        let snapshot = BrowserSnapshot {
            requested_url: Url::parse("https://example.com/").expect("url"),
            final_url: Url::parse("https://example.com/").expect("url"),
            document_html: "<html><body>hi</body></html>".into(),
            captured_at_epoch: 1,
            browser_engine: "fixture".into(),
            profile_id: "test".into(),
            challenge_state: PageChallengeState::Normal,
        };
        let artifact = snapshot_to_fetch_artifact(&snapshot);
        assert_eq!(artifact.content_type.as_deref(), Some("text/html"));
        assert_eq!(artifact.status, 200);
        assert!(artifact.body_bytes().is_some());
    }
}
