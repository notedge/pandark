use oak_core::parser::session::ParseSession;
use oak_core::{Parser, RedTree, SourceText, RedNode};
use oak_html::parser::element_type::HtmlElementType;
use oak_html::{HtmlLanguage, HtmlParser};

fn main() {
    let html = r#"<a href='/next'>x</a>"#;
    let source = SourceText::new(html);
    let language = HtmlLanguage::default();
    let mut session = ParseSession::<HtmlLanguage>::default();
    let parser = HtmlParser::new(&language);
    let output = parser.parse(&source, &[], &mut session);
    let green = output.result.expect("parse");
    let root = RedNode::new(green, 0);
    dump(root, &source, 0);
}

fn dump(node: RedNode<HtmlLanguage>, source: &SourceText, depth: usize) {
    let indent = "  ".repeat(depth);
    eprintln!("{indent}{:?} text={:?}", node.element_type(), node.text(source));
    for child in node.children() {
        match child {
            RedTree::Node(n) => dump(n, source, depth + 1),
            RedTree::Leaf(l) => {
                let kind = HtmlElementType::from(l.kind());
                eprintln!("{}  Leaf({:?}) text={:?}", indent, kind, l.text(source));
            }
        }
    }
}
