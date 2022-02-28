//! Extract from `src/html.rs`.

use std::collections::BTreeMap;

use notedown_ir::{Block, SourceKind};
use pandark_extract::{Extractor, HtmlExtractor};
use pandark_types::{
    BodyStorage, ExtractBudget, ExtractContext, ExtractInput, ExtractStatus, FetchArtifact, LinkRelation, TransportProvenance,
};
use url::Url;

fn artifact(page: &str, html: &str) -> FetchArtifact {
    FetchArtifact {
        request_url: Url::parse(page).expect("url"),
        final_url: Url::parse(page).expect("url"),
        status: 200,
        response_headers: BTreeMap::new(),
        content_type: Some("text/html".into()),
        body: BodyStorage::inline(html.as_bytes().to_vec()),
        redirect_chain: Vec::new(),
        retrieved_at_epoch: 1,
        transport_provenance: TransportProvenance::Test,
        cache_key: page.into(),
    }
}

#[test]
fn html_import_drives_title_blocks_assets_and_provenance() {
    let artifact = artifact(
        "https://example.com/docs/page.html",
        r#"<html><head><title>Guide</title></head><body><h1>Title</h1><p>Body <img src="../img/diagram.png" alt="D"></p></body></html>"#,
    );
    let result = HtmlExtractor
        .extract(ExtractInput { artifact: &artifact, budget: ExtractBudget::default() }, &ExtractContext::default());

    let document = result.document.expect("document");
    assert_eq!(document.metadata.title.as_deref(), Some("Guide"));
    assert!(matches!(document.blocks[0].block, Block::Section { .. }));
    assert_eq!(document.assets.len(), 1);
    assert_eq!(document.assets[0].source.as_deref(), Some("https://example.com/img/diagram.png"));
    assert_eq!(document.relations.len(), 1);
    assert!(document.sources.iter().all(|source| matches!(source.kind, SourceKind::TextSpan { .. })), "{:?}", document.sources);
    assert_eq!(result.status, ExtractStatus::Partial);
}

#[test]
fn html_links_resolve_against_page_base_and_keep_crawl_relations() {
    let artifact = artifact(
        "https://example.com/docs/page.html",
        r##"<html><head><base href="/guide/"></head><body><a href="next.html">Next</a><a href="#top">Top</a><a href="https://other.example/x">Out</a><img src="pic.png"></body></html>"##,
    );
    let result = HtmlExtractor
        .extract(ExtractInput { artifact: &artifact, budget: ExtractBudget::default() }, &ExtractContext::default());

    let links = result.discovered_links;
    assert!(
        links
            .iter()
            .any(|link| link.target == "https://example.com/guide/next.html" && link.relation == LinkRelation::Navigation),
        "{links:?}"
    );
    assert!(
        links.iter().any(|link| link.target == "https://other.example/x" && link.relation == LinkRelation::External),
        "{links:?}"
    );
    assert!(
        links.iter().any(|link| link.target == "https://example.com/guide/pic.png" && link.relation == LinkRelation::Asset),
        "{links:?}"
    );
    assert!(links.iter().all(|link| !link.target.contains("#top")), "{links:?}");
}
