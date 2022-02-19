use oak_core::parser::session::ParseSession;
use oak_core::{Parser, RedNode, RedTree, SourceText};
use oak_html::parser::element_type::HtmlElementType;
use oak_html::{HtmlLanguage, HtmlParser};
use pandark_types::CrawlError;
use url::Url;

/// Parsed HTML document retained for Oak red-tree traversal.
pub struct HtmlParse {
    source: SourceText,
    session: ParseSession<HtmlLanguage>,
}

impl HtmlParse {
    /// Parse HTML through `oak-html`.
    pub fn parse(html: &str) -> Result<Self, CrawlError> {
        let source = SourceText::new(html);
        let language = HtmlLanguage::default();
        let mut session = ParseSession::<HtmlLanguage>::default();
        let parser = HtmlParser::new(&language);
        let output = parser.parse(&source, &[], &mut session);
        output
            .result
            .map_err(|error| CrawlError::InvalidInput(error.to_string()))?;
        if session.last_root().is_none() {
            return Err(CrawlError::InvalidInput("oak-html produced no document root".into()));
        }
        Ok(Self { source, session })
    }

    fn root(&self) -> RedNode<'_, HtmlLanguage> {
        RedNode::new(self.session.last_root().expect("parsed root"), 0)
    }

    fn source(&self) -> &SourceText {
        &self.source
    }
}

/// Raw link target discovered in HTML with a source hint for reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LinkTarget {
    href: String,
    source_hint: &'static str,
}

/// Discover navigational href strings relative to the page URL and optional `<base>`.
pub fn extract_hrefs(html: &str, page_url: &Url) -> Vec<String> {
    match HtmlParse::parse(html) {
        Ok(parsed) => collect_link_targets(&parsed, page_url)
            .into_iter()
            .map(|target| target.href)
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Discover links and preserve the HTML attribute that produced each target.
pub(crate) fn extract_link_targets(
    html: &str,
    page_url: &Url,
) -> Vec<(String, &'static str)> {
    match HtmlParse::parse(html) {
        Ok(parsed) => collect_link_targets(&parsed, page_url)
            .into_iter()
            .map(|target| (target.href, target.source_hint))
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Document title from the first `<title>` element.
pub fn extract_document_title(html: &str) -> Option<String> {
    let parsed = HtmlParse::parse(html).ok()?;
    let source = parsed.source();
    let title_node = find_first_element_by_tag(parsed.root(), source, "title")?;
    let text = collect_element_text(title_node, source);
    let collapsed = collapse_whitespace(text);
    if collapsed.is_empty() {
        None
    } else {
        Some(collapsed)
    }
}

/// Visible body text with `script`/`style`/`noscript` content excluded.
pub fn extract_body_text(html: &str) -> String {
    match HtmlParse::parse(html) {
        Ok(parsed) => {
            let source = parsed.source();
            let scope = find_first_element_by_tag(parsed.root(), source, "body")
                .unwrap_or_else(|| parsed.root());
            collapse_whitespace(collect_element_text(scope, source))
        }
        Err(_) => String::new(),
    }
}

fn document_base_url(page_url: &Url, parsed: &HtmlParse) -> Url {
    let source = parsed.source();
    let base_href = find_first_element_by_tag(parsed.root(), source, "base")
        .and_then(|node| attribute_value(node, source, "href"));
    resolve_base_url(page_url, base_href.as_deref())
}

fn resolve_base_url(page_url: &Url, base_href: Option<&str>) -> Url {
    let Some(href) = base_href else {
        return page_url.clone();
    };
    if let Ok(absolute) = Url::parse(href) {
        return absolute;
    }
    page_url.join(href).unwrap_or_else(|_| page_url.clone())
}

fn collect_link_targets(parsed: &HtmlParse, page_url: &Url) -> Vec<LinkTarget> {
    let source = parsed.source();
    let base_url = document_base_url(page_url, parsed);
    let mut targets = Vec::new();
    walk_elements(parsed.root(), source, &mut |node, tag| {
        match tag.as_str() {
            "a" => push_href_target(node, source, "href", "href", &base_url, &mut targets),
            "form" => push_href_target(node, source, "action", "form-action", &base_url, &mut targets),
            "img" | "source" => {
                push_href_target(node, source, "src", "src", &base_url, &mut targets);
                if let Some(srcset) = attribute_value(node, source, "srcset") {
                    for url in parse_srcset(&srcset) {
                        targets.push(LinkTarget {
                            href: normalize_href(&base_url, &url),
                            source_hint: "srcset",
                        });
                    }
                }
            }
            _ => {}
        }
    });
    targets
}

fn push_href_target(
    node: RedNode<HtmlLanguage>,
    source: &SourceText,
    attr: &str,
    hint: &'static str,
    page_url: &Url,
    targets: &mut Vec<LinkTarget>,
) {
    if let Some(href) = attribute_value(node, source, attr) {
        if is_navigable_href(&href) {
            targets.push(LinkTarget {
                href: normalize_href(page_url, &href),
                source_hint: hint,
            });
        }
    }
}

fn normalize_href(page_url: &Url, href: &str) -> String {
    if let Ok(absolute) = Url::parse(href) {
        return absolute.to_string();
    }
    page_url
        .join(href)
        .map(|url| url.to_string())
        .unwrap_or_else(|_| href.to_string())
}

fn is_navigable_href(href: &str) -> bool {
    let trimmed = href.trim();
    !trimmed.is_empty()
        && !trimmed.starts_with('#')
        && !trimmed.starts_with("javascript:")
        && !trimmed.starts_with("mailto:")
        && !trimmed.starts_with("data:")
}

fn parse_srcset(value: &str) -> Vec<String> {
    value
        .split(',')
        .filter_map(|part| {
            let url = part.trim().split_whitespace().next().unwrap_or("").trim();
            if url.is_empty() {
                None
            } else {
                Some(url.to_string())
            }
        })
        .collect()
}

fn walk_elements<'a>(
    node: RedNode<'a, HtmlLanguage>,
    source: &'a SourceText,
    visit: &mut dyn FnMut(RedNode<'a, HtmlLanguage>, String),
) {
    if node.element_type() == HtmlElementType::Element {
        if let Some(tag) = element_tag_name(node, source) {
            if is_skipped_subtree_tag(&tag) {
                return;
            }
            visit(node, tag);
        }
    }
    for child in node.children() {
        if let RedTree::Node(child_node) = child {
            walk_elements(child_node, source, visit);
        }
    }
}

fn find_first_element_by_tag<'a>(
    node: RedNode<'a, HtmlLanguage>,
    source: &'a SourceText,
    tag: &str,
) -> Option<RedNode<'a, HtmlLanguage>> {
    if node.element_type() == HtmlElementType::Element {
        if element_tag_name(node, source).as_deref() == Some(tag) {
            return Some(node);
        }
    }
    for child in node.children() {
        if let RedTree::Node(child_node) = child {
            if let Some(found) = find_first_element_by_tag(child_node, source, tag) {
                return Some(found);
            }
        }
    }
    None
}

fn element_tag_name(node: RedNode<HtmlLanguage>, source: &SourceText) -> Option<String> {
    if node.element_type() != HtmlElementType::Element {
        return None;
    }
    for child in node.children() {
        match child {
            RedTree::Node(child_node) if child_node.element_type() == HtmlElementType::TagName => {
                let name = child_node.text(source).trim().to_ascii_lowercase();
                if !name.is_empty() {
                    return Some(name);
                }
            }
            RedTree::Leaf(leaf) if leaf_kind(&leaf) == HtmlElementType::TagName => {
                let name = leaf.text(source).trim().to_ascii_lowercase();
                if !name.is_empty() {
                    return Some(name);
                }
            }
            _ => {}
        }
    }
    None
}

fn attribute_value(
    node: RedNode<HtmlLanguage>,
    source: &SourceText,
    name: &str,
) -> Option<String> {
    for child in node.children() {
        let attr_node = match child {
            RedTree::Node(child_node) if child_node.element_type() == HtmlElementType::Attribute => {
                child_node
            }
            _ => continue,
        };
        let mut attr_name = None;
        let mut attr_value = None;
        for attr_child in attr_node.children() {
            match attr_child {
                RedTree::Node(attr_part) => match attr_part.element_type() {
                    HtmlElementType::AttributeName | HtmlElementType::TagName => {
                        attr_name = Some(attr_part.text(source).trim().to_ascii_lowercase());
                    }
                    HtmlElementType::AttributeValue => {
                        attr_value = Some(unquote(attr_part.text(source).as_ref()));
                    }
                    _ => {}
                },
                RedTree::Leaf(leaf) => match leaf_kind(&leaf) {
                    HtmlElementType::AttributeName | HtmlElementType::TagName => {
                        attr_name = Some(leaf.text(source).trim().to_ascii_lowercase());
                    }
                    HtmlElementType::AttributeValue => {
                        attr_value = Some(unquote(leaf.text(source).as_ref()));
                    }
                    _ => {}
                },
            }
        }
        if attr_name.as_deref() == Some(name) {
            return attr_value;
        }
    }
    None
}

fn collect_element_text<'a>(node: RedNode<'a, HtmlLanguage>, source: &'a SourceText) -> String {
    let mut parts = Vec::new();
    collect_text_parts(node, source, &mut parts);
    parts.join("")
}

fn collect_text_parts<'a>(
    node: RedNode<'a, HtmlLanguage>,
    source: &'a SourceText,
    parts: &mut Vec<String>,
) {
    if node.element_type() == HtmlElementType::Element {
        if let Some(tag) = element_tag_name(node, source) {
            if is_skipped_text_tag(&tag) {
                return;
            }
        }
    }

    for child in node.children() {
        match child {
            RedTree::Leaf(leaf) => match leaf_kind(&leaf) {
                HtmlElementType::Text | HtmlElementType::Whitespace | HtmlElementType::EntityRef | HtmlElementType::CharRef => {
                    parts.push(decode_html_entities(leaf.text(source).as_ref()));
                }
                HtmlElementType::TagSlashOpen => return,
                HtmlElementType::Comment => {}
                _ => {}
            },
            RedTree::Node(child_node) => match child_node.element_type() {
                HtmlElementType::TagSlashOpen => return,
                HtmlElementType::Comment => {}
                HtmlElementType::Text => {
                    parts.push(decode_html_entities(child_node.text(source).as_ref()));
                }
                HtmlElementType::Element => {
                    if let Some(tag) = element_tag_name(child_node, source) {
                        if is_skipped_text_tag(&tag) {
                            continue;
                        }
                    }
                    collect_text_parts(child_node, source, parts);
                }
                HtmlElementType::Document => collect_text_parts(child_node, source, parts),
                _ => {}
            },
        }
    }
}

fn leaf_kind(leaf: &oak_core::tree::RedLeaf<HtmlLanguage>) -> HtmlElementType {
    HtmlElementType::from(leaf.kind())
}

fn is_skipped_text_tag(tag: &str) -> bool {
    matches!(tag, "script" | "style" | "noscript" | "template")
}

fn is_skipped_subtree_tag(tag: &str) -> bool {
    matches!(tag, "script" | "style" | "noscript" | "template" | "comment")
}

fn unquote(value: &str) -> String {
    let trimmed = value.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

fn collapse_whitespace(input: String) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn decode_html_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '&' {
            out.push(ch);
            continue;
        }
        let mut entity = String::from('&');
        while let Some(&next) = chars.peek() {
            if next == ';' {
                entity.push(chars.next().expect("semicolon"));
                break;
            }
            if next.is_whitespace() || next == '<' {
                break;
            }
            entity.push(chars.next().expect("entity char"));
            if entity.len() > 32 {
                break;
            }
        }
        out.push_str(resolve_entity(&entity));
    }
    out
}

fn resolve_entity(entity: &str) -> &str {
    match entity {
        "&amp;" => "&",
        "&lt;" => "<",
        "&gt;" => ">",
        "&quot;" => "\"",
        "&#39;" | "&apos;" => "'",
        _ => entity,
    }
}

