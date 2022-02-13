use notedown_ir::{
    Asset, AssetKind, Block, DocumentGraph, DocumentId, DocumentMetadata, IdAllocator, Inline,
    LossMarker, Relation, RelationEndpoint, RelationKind, SemanticStatus, SourceKind, SourceRef,
};
use pandark_types::{
    ExtractBudget, ExtractContext, ExtractInput, ExtractResult, ExtractStatus, FetchArtifact,
    ProbeResult, ProbeStatus,
};

use super::oak_view;
use super::{link_candidates_from_targets, Extractor};

/// HTML extractor backed by Oak HTML parsing and traversal.
#[derive(Debug, Clone, Copy, Default)]
pub struct HtmlExtractor;

impl Extractor for HtmlExtractor {
    fn probe(&self, artifact: &FetchArtifact, budget: ExtractBudget) -> ProbeResult {
        let supported = artifact
            .content_type
            .as_deref()
            .map(|value| value.contains("html"))
            .unwrap_or(false);
        if !supported {
            return ProbeResult {
                status: ProbeStatus::Unsupported,
                extractor: None,
            };
        }
        let bytes = artifact.body_bytes().unwrap_or(&[]);
        if bytes.len() as u64 > budget.max_probe_bytes {
            return ProbeResult {
                status: ProbeStatus::Failed,
                extractor: Some("html".into()),
            };
        }
        ProbeResult {
            status: ProbeStatus::Supported,
            extractor: Some("html".into()),
        }
    }

    fn extract(&self, input: ExtractInput<'_>, ctx: &ExtractContext) -> ExtractResult {
        let artifact = input.artifact;
        let body = artifact.body_bytes().unwrap_or(&[]);
        let html = String::from_utf8_lossy(body);
        let title = oak_view::extract_document_title(&html);
        let text = oak_view::extract_body_text(&html);
        let targets = oak_view::extract_link_targets(&html, &artifact.final_url);
        let discovered_links = link_candidates_from_targets(&artifact.final_url, &targets);
        let image_targets = targets
            .iter()
            .filter(|(_, hint)| matches!(*hint, "src" | "srcset"))
            .map(|(target, _)| target.clone())
            .collect::<Vec<_>>();

        let page_identity = ctx
            .page_identity
            .clone()
            .unwrap_or_else(|| artifact.final_url.to_string());
        let mut document = DocumentGraph::new(page_document_id(&page_identity));
        let mut ids = IdAllocator::default();
        document.metadata = DocumentMetadata {
            title,
            language: None,
            authors: Vec::new(),
            tags: Vec::new(),
        };
        if !text.is_empty() {
            let node = document.push_block(Block::Paragraph {
                content: vec![Inline::Text { text }],
            });
            document.attach_source(SourceRef {
                node,
                kind: SourceKind::Synthetic {
                    reason: format!("HTML body from {}", artifact.final_url),
                },
                precision: SemanticStatus::Resolved,
            });
        } else {
            document.push_loss(LossMarker {
                code: "pandark.extract.empty-body".into(),
                message: "HTML body had no extractable text".into(),
                status: SemanticStatus::Partial,
            });
        }

        for source in image_targets {
            let asset_id = ids.asset_id();
            let node = document.push_block(Block::Paragraph {
                content: vec![Inline::Styled {
                    style: "image".into(),
                    children: vec![
                        Inline::Text {
                            text: source.rsplit('/').next().unwrap_or("image").to_string(),
                        },
                        Inline::Text {
                            text: source.clone(),
                        },
                    ],
                }],
            });
            document.push_asset(Asset {
                id: asset_id,
                kind: AssetKind::Image,
                content_identity: None,
                source: Some(source.clone()),
                media_type: media_type_for_url(&source),
                status: SemanticStatus::Unresolved,
                bytes: None,
            });
            document.push_relation(Relation {
                id: ids.link_id(),
                kind: RelationKind::Embeds,
                source: RelationEndpoint::Node(node),
                target: RelationEndpoint::Asset(asset_id),
            });
            document.attach_source(SourceRef {
                node,
                kind: SourceKind::Synthetic {
                    reason: format!("HTML image reference from {}", artifact.final_url),
                },
                precision: SemanticStatus::Resolved,
            });
        }

        let validation = document.validate();
        let losses = document.coverage.loss.clone();
        let status = if validation.is_valid() {
            if document.coverage.complete {
                ExtractStatus::Complete
            } else {
                ExtractStatus::Partial
            }
        } else {
            ExtractStatus::Failed
        };

        ExtractResult {
            status,
            source_url: artifact.final_url.clone(),
            document: if status == ExtractStatus::Failed {
                None
            } else {
                Some(document)
            },
            discovered_links,
            losses,
            extractor: "html".into(),
        }
    }
}

fn media_type_for_url(url: &str) -> Option<String> {
    let extension = url
        .split('?')
        .next()
        .and_then(|value| value.rsplit('.').next())?
        .to_ascii_lowercase();
    let media_type = match extension.as_str() {
        "avif" => "image/avif",
        "gif" => "image/gif",
        "jpeg" | "jpg" => "image/jpeg",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        _ => return None,
    };
    Some(media_type.into())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use pandark_types::{BodyStorage, ExtractContext, ExtractInput, TransportProvenance};
    use url::Url;

    use super::*;

    #[test]
    fn image_sources_become_ir_assets_and_embed_relations() {
        let page = Url::parse("https://example.com/docs/page.html").expect("url");
        let artifact = FetchArtifact {
            request_url: page.clone(),
            final_url: page,
            status: 200,
            response_headers: BTreeMap::new(),
            content_type: Some("text/html".into()),
            body: BodyStorage::inline(
                br#"<html><body><p>Body</p><img src="../img/diagram.png" srcset="/img/large.webp 2x"></body></html>"#,
            ),
            redirect_chain: Vec::new(),
            retrieved_at_epoch: 1,
            transport_provenance: TransportProvenance::Test,
            cache_key: "page".into(),
        };
        let result = HtmlExtractor.extract(
            ExtractInput {
                artifact: &artifact,
                budget: ExtractBudget::default(),
            },
            &ExtractContext::default(),
        );
        let document = result.document.expect("document");
        assert_eq!(document.assets.len(), 2);
        assert_eq!(document.relations.len(), 2);
        assert!(document
            .assets
            .iter()
            .any(|asset| asset.media_type.as_deref() == Some("image/png")));
        assert!(document
            .sources
            .iter()
            .any(|source| matches!(source.kind, SourceKind::Synthetic { .. })));
    }
}
fn page_document_id(page_identity: &str) -> DocumentId {
    let mut hash = 0u64;
    for byte in page_identity.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }
    DocumentId(hash)
}
