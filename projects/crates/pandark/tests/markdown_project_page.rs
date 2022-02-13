use std::fs;

use notedown_formats::import::markdown::import_markdown_bytes;
use notedown_ir::{Block, Inline};
use pandark::{HtmlExtractor, publish_extracted_page, run_extract};
use pandark_types::{
    BodyStorage, ExtractBudget, ExtractContext, ExtractStatus, FetchArtifact, TransportProvenance,
};
use url::Url;

fn fixture_artifact(html: &str, page: &str) -> FetchArtifact {
    FetchArtifact {
        request_url: Url::parse(page).expect("url"),
        final_url: Url::parse(page).expect("url"),
        status: 200,
        response_headers: Default::default(),
        content_type: Some("text/html".into()),
        body: BodyStorage::inline(html.as_bytes()),
        redirect_chain: Vec::new(),
        retrieved_at_epoch: 1,
        transport_provenance: TransportProvenance::Test,
        cache_key: page.to_string(),
    }
}

#[test]
fn html_page_publishes_markdown_project_with_reopen() {
    let html = "<html><head><title>Sample</title></head><body><p>Hello Pandark page</p></body></html>";
    let artifact = fixture_artifact(html, "https://example.com/article");
    let extractor = HtmlExtractor;
    let extractors: [&dyn pandark::Extractor; 1] = [&extractor];
    let result = run_extract(
        &artifact,
        &extractors,
        ExtractBudget::default(),
        &ExtractContext::default(),
    );
    assert!(matches!(result.status, ExtractStatus::Partial | ExtractStatus::Complete));

    let output_dir = std::env::temp_dir().join(format!("pandark-page-project-{}", std::process::id()));
    let published = publish_extracted_page(&output_dir, "article.html", &result).expect("publish page project");

    assert!(published.index_path.exists());
    assert!(published.report_path.ends_with("panduck.report.json"));
    let index = fs::read_to_string(published.index_path).expect("read index.md");
    assert!(index.contains("Hello Pandark page"));

    let report = fs::read_to_string(published.report_path).expect("read report");
    assert!(report.contains("extract_page"));
    assert!(report.contains("https://example.com/article"));

    let reopened = import_markdown_bytes("index.md", &index).expect("reopen markdown");
    assert!(reopened.blocks.iter().any(|node| matches!(
        &node.block,
        Block::Paragraph { content }
            if content.iter().any(|inline| matches!(inline, Inline::Text { text } if text.contains("Hello Pandark page")))
    )));

    let _ = fs::remove_dir_all(output_dir);
}
