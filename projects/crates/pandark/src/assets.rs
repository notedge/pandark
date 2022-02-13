use blake3::Hash;
use pandark_fetch::{FetchClient, FetchRequest, Transport};
use pandark_types::Result;
use notedown_ir::{DocumentGraph, LossMarker, SemanticStatus};
use std::collections::BTreeMap;
use url::Url;

/// Download and materialize unresolved document assets through the fetch layer.
pub fn hydrate_assets<T: Transport>(
    document: &mut DocumentGraph,
    client: &mut FetchClient<T>,
    max_assets: usize,
    max_bytes: u64,
) -> Result<usize> {
    let sources = document
        .assets
        .iter()
        .filter(|asset| asset.status == SemanticStatus::Unresolved)
        .filter_map(|asset| asset.source.clone().map(|source| (asset.id, source)))
        .collect::<Vec<_>>();
    if sources.len() > max_assets {
        document.push_loss(asset_loss(
            "budget_exhausted",
            "document assets",
            format!("asset count budget exceeded: {} candidates, {max_assets} allowed", sources.len()),
        ));
    }
    let mut used_bytes = 0u64;
    let mut hydrated = 0;

    for (asset_id, source) in sources.into_iter().take(max_assets) {
        let url = match Url::parse(&source) {
            Ok(url) => url,
            Err(error) => {
                document.push_loss(asset_loss("invalid_url", &source, error.to_string()));
                continue;
            }
        };
        let artifact = match client.fetch(&FetchRequest {
            url,
            headers: BTreeMap::new(),
        }) {
            Ok(artifact) => artifact,
            Err(error) => {
                document.push_loss(asset_loss("fetch_failed", &source, error.to_string()));
                continue;
            }
        };
        if !(200..300).contains(&artifact.status) {
            document.push_loss(asset_loss(
                "http_status",
                &source,
                format!("asset response returned HTTP {}", artifact.status),
            ));
            continue;
        }
        let Some(bytes) = artifact.body_bytes() else {
            document.push_loss(asset_loss("body_unavailable", &source, "fetch body is out-of-line"));
            continue;
        };
        let size = bytes.len() as u64;
        if used_bytes.saturating_add(size) > max_bytes {
            document.push_loss(asset_loss("budget_exhausted", &source, "asset byte budget exceeded"));
            continue;
        }
        let fingerprint = fingerprint(bytes);
        if let Some(asset) = document.assets.iter_mut().find(|asset| asset.id == asset_id) {
            asset.bytes = Some(bytes.to_vec());
            asset.content_identity = Some(fingerprint);
            asset.media_type = artifact.content_type.or(asset.media_type.clone());
            asset.status = SemanticStatus::Resolved;
            used_bytes += size;
            hydrated += 1;
        }
    }
    Ok(hydrated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use notedown_ir::{Asset, AssetId, AssetKind, DocumentId};
    use pandark_fetch::{MemoryEntry, MemoryTransport, RobotsPolicy};
    use std::collections::BTreeMap;

    fn document_with_asset(source: &str) -> DocumentGraph {
        let mut document = DocumentGraph::new(DocumentId(1));
        document.push_asset(Asset {
            id: AssetId(1),
            kind: AssetKind::Image,
            content_identity: None,
            source: Some(source.to_string()),
            media_type: Some("image/jpeg".to_string()),
            status: SemanticStatus::Unresolved,
            bytes: None,
        });
        document
    }

    #[test]
    fn materializes_bytes_identity_and_response_media_type() {
        let source = "https://example.com/image.bin";
        let mut transport = MemoryTransport::new();
        transport.insert(
            source,
            MemoryEntry {
                status: 200,
                headers: BTreeMap::from([("content-type".into(), "image/png".into())]),
                body: b"asset-body".to_vec(),
            },
        );
        let mut client = FetchClient::new(
            transport,
            RobotsPolicy::from_profile(pandark_types::PolitenessProfile::Developer),
        );
        let mut document = document_with_asset(source);

        let count = hydrate_assets(&mut document, &mut client, 8, 1024).expect("hydrate");

        assert_eq!(count, 1);
        let asset = &document.assets[0];
        let expected_identity = fingerprint(b"asset-body");
        assert_eq!(asset.bytes.as_deref(), Some(b"asset-body".as_slice()));
        assert_eq!(asset.media_type.as_deref(), Some("image/png"));
        assert_eq!(asset.status, SemanticStatus::Resolved);
        assert_eq!(asset.content_identity.as_deref(), Some(expected_identity.as_str()));
        assert!(document.coverage.complete);
    }

    #[test]
    fn fetch_failure_preserves_unresolved_asset_and_records_loss() {
        let source = "https://example.com/missing.png";
        let mut client = FetchClient::new(
            MemoryTransport::new(),
            RobotsPolicy::from_profile(pandark_types::PolitenessProfile::Developer),
        );
        let mut document = document_with_asset(source);

        assert_eq!(hydrate_assets(&mut document, &mut client, 8, 1024).expect("hydrate"), 0);
        assert_eq!(document.assets[0].status, SemanticStatus::Unresolved);
        assert_eq!(document.coverage.loss[0].code, "pandark.asset.fetch_failed");
        assert!(!document.coverage.complete);
    }

    #[test]
    fn byte_budget_records_loss_without_materializing_asset() {
        let source = "https://example.com/large.png";
        let mut transport = MemoryTransport::new();
        transport.insert(
            source,
            MemoryEntry {
                status: 200,
                headers: BTreeMap::new(),
                body: b"12345".to_vec(),
            },
        );
        let mut client = FetchClient::new(
            transport,
            RobotsPolicy::from_profile(pandark_types::PolitenessProfile::Developer),
        );
        let mut document = document_with_asset(source);

        assert_eq!(hydrate_assets(&mut document, &mut client, 8, 4).expect("hydrate"), 0);
        assert_eq!(document.assets[0].status, SemanticStatus::Unresolved);
        assert_eq!(document.coverage.loss[0].code, "pandark.asset.budget_exhausted");
    }

    #[test]
    fn non_success_response_records_loss_without_resolving_asset() {
        let source = "https://example.com/missing.png";
        let mut transport = MemoryTransport::new();
        transport.insert(
            source,
            MemoryEntry {
                status: 404,
                headers: BTreeMap::new(),
                body: b"not found".to_vec(),
            },
        );
        let mut client = FetchClient::new(
            transport,
            RobotsPolicy::from_profile(pandark_types::PolitenessProfile::Developer),
        );
        let mut document = document_with_asset(source);

        assert_eq!(hydrate_assets(&mut document, &mut client, 8, 1024).expect("hydrate"), 0);
        assert_eq!(document.assets[0].status, SemanticStatus::Unresolved);
        assert_eq!(document.coverage.loss[0].code, "pandark.asset.http_status");
    }

    #[test]
    fn zero_asset_budget_records_loss_without_fetching() {
        let mut document = document_with_asset("https://example.com/image.png");
        let mut client = FetchClient::new(
            MemoryTransport::new(),
            RobotsPolicy::from_profile(pandark_types::PolitenessProfile::Developer),
        );
        assert_eq!(hydrate_assets(&mut document, &mut client, 0, 1024).expect("hydrate"), 0);
        assert_eq!(document.assets[0].status, SemanticStatus::Unresolved);
        assert_eq!(document.coverage.loss.len(), 1);
        assert_eq!(document.coverage.loss[0].code, "pandark.asset.budget_exhausted");
        assert!(!document.coverage.complete);
    }
}

fn fingerprint(bytes: &[u8]) -> String {
    let hash: Hash = blake3::hash(bytes);
    hash.to_hex().to_string()
}

fn asset_loss(code: &str, source: &str, message: impl Into<String>) -> LossMarker {
    LossMarker {
        code: format!("pandark.asset.{code}"),
        message: format!("{source}: {}", message.into()),
        status: SemanticStatus::Partial,
    }
}
