//! Extracted from `src/oak_view.rs`.

use pandark_extract::{extract_body_text, extract_document_title, extract_hrefs};
use url::Url;

#[test]
fn title_is_case_insensitive() {
    let html = "<html><HEAD><TITLE>Case &amp; Title</TITLE></HEAD><body></body></html>";
    assert_eq!(
        extract_document_title(html).as_deref(),
        Some("Case & Title")
    );
}

#[test]
fn href_supports_single_quotes() {
    let page = Url::parse("https://example.com/docs/").expect("url");
    let html = r#"<html><body><a href='/next'>x</a></body></html>"#;
    let hrefs = extract_hrefs(html, &page);
    assert_eq!(hrefs, vec!["https://example.com/next"]);
}

#[test]
fn script_text_is_excluded_from_body() {
    let html = "<html><body><script>alert(1)</script><p>Hello</p></body></html>";
    assert_eq!(extract_body_text(html), "Hello");
}

#[test]
fn base_href_rebases_relative_links() {
    let page = Url::parse("https://example.com/docs/page.html").expect("url");
    let html = r#"<html><head><base href="https://cdn.example.com/sub/"></head>
<body><a href="item.html">x</a></body></html>"#;
    let hrefs = extract_hrefs(html, &page);
    assert_eq!(hrefs, vec!["https://cdn.example.com/sub/item.html"]);
}

#[test]
fn collects_form_action_and_srcset() {
    let page = Url::parse("https://example.com/").expect("url");
    let html = r#"<html><body>
<form action="/search"></form>
<img src="/a.png" srcset="/b.png 1x, https://cdn.example/x.webp 2x">
</body></html>"#;
    let hrefs = extract_hrefs(html, &page);
    assert!(hrefs.contains(&"https://example.com/search".to_string()));
    assert!(hrefs.contains(&"https://example.com/a.png".to_string()));
    assert!(hrefs.contains(&"https://example.com/b.png".to_string()));
    assert!(hrefs.contains(&"https://cdn.example/x.webp".to_string()));
}

#[test]
fn html_comment_does_not_break_link_discovery() {
    let page = Url::parse("https://example.com/").expect("url");
    let html = r#"<html><body><!-- <a href="/trap"> --><a href="/ok">ok</a></body></html>"#;
    let hrefs = extract_hrefs(html, &page);
    assert_eq!(hrefs, vec!["https://example.com/ok"]);
}
