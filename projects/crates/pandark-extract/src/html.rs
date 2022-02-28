use notedown_ir::{Block, DocumentGraph, DocumentId, Inline, LossMarker, SemanticStatus};
use pandark_types::{
    ExtractBudget, ExtractContext, ExtractInput, ExtractResult, ExtractStatus, FetchArtifact, ProbeResult, ProbeStatus,
};

use super::{Extractor, import_html_page, link_candidates_from_import, resolve_base_url};

/// HTML extractor backed by the `notedown-formats` HTML import.
///
/// The format layer owns document lowering, provenance, and loss. This
/// extractor only applies crawl policy: base URI resolution for assets and
/// navigation candidate classification.
#[derive(Debug, Clone, Copy, Default)]
pub struct HtmlExtractor;

impl Extractor for HtmlExtractor {
    fn probe(&self, artifact: &FetchArtifact, budget: ExtractBudget) -> ProbeResult {
        let supported = artifact.content_type.as_deref().map(|value| value.contains("html")).unwrap_or(false);
        if !supported {
            return ProbeResult { status: ProbeStatus::Unsupported, extractor: None };
        }
        let bytes = artifact.body_bytes().unwrap_or(&[]);
        if bytes.len() as u64 > budget.max_probe_bytes {
            return ProbeResult { status: ProbeStatus::Failed, extractor: Some("html".into()) };
        }
        ProbeResult { status: ProbeStatus::Supported, extractor: Some("html".into()) }
    }

    fn extract(&self, input: ExtractInput<'_>, ctx: &ExtractContext) -> ExtractResult {
        let artifact = input.artifact;
        let label = artifact.final_url.to_string();
        let html = String::from_utf8_lossy(artifact.body_bytes().unwrap_or(&[]));
        let Some(import) = import_html_page(&artifact.final_url, &html)
        else {
            return ExtractResult {
                status: ExtractStatus::Failed,
                source_url: artifact.final_url.clone(),
                document: None,
                discovered_links: Vec::new(),
                losses: vec![LossMarker {
                    code: "pandark.extract.html_import_failed".into(),
                    message: format!("`notedown-formats` could not import HTML from {label}"),
                    status: SemanticStatus::Partial,
                }],
                extractor: "html".into(),
            };
        };

        let discovered_links = link_candidates_from_import(&artifact.final_url, import.base_href.as_deref(), &import.links);
        let base_url = resolve_base_url(&artifact.final_url, import.base_href.as_deref());
        let mut document = import.document;
        document.id = page_document_id(ctx.page_identity.as_deref().unwrap_or(&label));
        resolve_asset_targets(&mut document, &base_url);

        let status = if document.validate().is_valid() {
            if document.coverage.complete { ExtractStatus::Complete } else { ExtractStatus::Partial }
        }
        else {
            ExtractStatus::Failed
        };
        let losses = document.coverage.loss.clone();

        ExtractResult {
            status,
            source_url: artifact.final_url.clone(),
            document: if status == ExtractStatus::Failed { None } else { Some(document) },
            discovered_links,
            losses,
            extractor: "html".into(),
        }
    }
}

/// Resolve asset targets against the document base URI so downloads and
/// reports see absolute locations. The format layer keeps authored targets.
fn resolve_asset_targets(document: &mut DocumentGraph, base_url: &url::Url) {
    for asset in &mut document.assets {
        let Some(source) = asset.source.clone()
        else {
            continue;
        };
        if let Some(resolved) = resolve_target(base_url, &source) {
            asset.source = Some(resolved);
        }
    }
    for node in &mut document.blocks {
        resolve_block_targets(&mut node.block, base_url);
    }
}

fn resolve_block_targets(block: &mut Block, base_url: &url::Url) {
    match block {
        Block::Section { title, .. } => resolve_inline_targets(title, base_url),
        Block::Paragraph { content } | Block::Quote { content } => resolve_inline_targets(content, base_url),
        Block::List { items, .. } => {
            for item in items {
                resolve_inline_targets(&mut item.content, base_url);
            }
        }
        Block::Table { rows } => {
            for row in rows {
                for cell in &mut row.cells {
                    resolve_inline_targets(cell, base_url);
                }
            }
        }
        _ => {}
    }
}

fn resolve_inline_targets(inlines: &mut [Inline], base_url: &url::Url) {
    for inline in inlines {
        match inline {
            Inline::Image { source, .. } if !source.is_empty() => {
                if let Some(resolved) = resolve_target(base_url, source) {
                    *source = resolved;
                }
            }
            Inline::Styled { children, .. } | Inline::Attributed { children, .. } => {
                resolve_inline_targets(children, base_url);
            }
            _ => {}
        }
    }
}

fn resolve_target(base_url: &url::Url, target: &str) -> Option<String> {
    url::Url::parse(target).or_else(|_| base_url.join(target)).ok().map(|url| url.to_string())
}

fn page_document_id(page_identity: &str) -> DocumentId {
    let mut hash = 0u64;
    for byte in page_identity.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }
    DocumentId(hash)
}
