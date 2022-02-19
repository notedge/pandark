//! Extracted from `src/html.rs`.

use std::collections::BTreeMap;

use notedown_ir::SourceKind;
use pandark_extract::{Extractor, HtmlExtractor};
use pandark_types::{
    BodyStorage, ExtractBudget, ExtractContext, ExtractInput, FetchArtifact, TransportProvenance,
};
use url::Url;

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
