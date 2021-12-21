use notedown_ir::{
    Block, DocumentGraph, DocumentId, DocumentMetadata, Inline, LossMarker, SemanticStatus,
};
use pandark_types::{
    ExtractBudget, ExtractContext, ExtractInput, ExtractResult, ExtractStatus, FetchArtifact,
    ProbeResult, ProbeStatus,
};

use super::{Extractor, link_candidates_from_hrefs};

/// HTML extractor using lightweight string heuristics until Oak lowering lands.
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
        let title = extract_title(&html);
        let text = extract_body_text(&html);
        let hrefs = extract_hrefs(&html);
        let discovered_links = link_candidates_from_hrefs(&artifact.final_url, &hrefs);

        let page_identity = ctx
            .page_identity
            .clone()
            .unwrap_or_else(|| artifact.final_url.to_string());
        let mut document = DocumentGraph::new(page_document_id(&page_identity));
        document.metadata = DocumentMetadata {
            title,
            language: None,
            authors: Vec::new(),
            tags: Vec::new(),
        };
        if !text.is_empty() {
            document.push_block(Block::Paragraph {
                content: vec![Inline::Text { text }],
            });
        } else {
            document.push_loss(LossMarker {
                code: "pandark.extract.empty-body".into(),
                message: "HTML body had no extractable text".into(),
                status: SemanticStatus::Partial,
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

fn page_document_id(page_identity: &str) -> DocumentId {
    let mut hash = 0u64;
    for byte in page_identity.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }
    DocumentId(hash)
}

fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title>")? + "<title>".len();
    let end = lower[start..].find("</title>")? + start;
    let raw = html.get(start..end)?.trim();
    if raw.is_empty() {
        None
    } else {
        Some(raw.to_string())
    }
}

fn extract_body_text(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    collapse_whitespace(out)
}

fn collapse_whitespace(input: String) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_hrefs(html: &str) -> Vec<String> {
    let mut hrefs = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut cursor = 0;
    while let Some(index) = lower[cursor..].find("href=\"") {
        let start = cursor + index + "href=\"".len();
        if let Some(end) = lower[start..].find('"') {
            let href = html[start..start + end].trim();
            if !href.is_empty() && !href.starts_with('#') && !href.starts_with("javascript:") {
                hrefs.push(href.to_string());
            }
            cursor = start + end + 1;
        } else {
            break;
        }
    }
    hrefs
}
