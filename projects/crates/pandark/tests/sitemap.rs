//! Extracted from `src/sitemap.rs`.

use std::io::Write;
use pandark_fetch::build_artifact;
use pandark_types::{BodyStorage, TransportProvenance};
use url::Url;

use pandark::{locations, sitemap_body, MAX_SITEMAP_LOCATIONS};

#[test]
fn rejects_unsupported_root_and_nested_location_payloads() {
    assert!(locations(b"<html><body>not a sitemap</body></html>").is_err());
    assert!(locations(b"<urlset><url><loc><span>https://example.com/</span></loc></url></urlset>").is_err());
    assert!(locations(&[0xff]).is_err());
}

#[test]
fn rejects_xml_above_the_parse_budget() {
    assert!(locations(&vec![b' '; 1024 * 1024 + 1]).is_err());
}

#[test]
fn rejects_excessive_location_count() {
    let mut xml = String::from("<urlset>");
    for index in 0..=MAX_SITEMAP_LOCATIONS {
        xml.push_str(&format!("<url><loc>https://example.com/{index}</loc></url>"));
    }
    xml.push_str("</urlset>");
    assert!(locations(xml.as_bytes()).is_err());
}

#[test]
fn decodes_gzip_and_rejects_unknown_encoding() {
    let mut compressed = Vec::new();
    let mut encoder = flate2::write::GzEncoder::new(&mut compressed, flate2::Compression::fast());
    encoder.write_all(b"<urlset/>").expect("compress");
    encoder.finish().expect("finish");
    let url = Url::parse("https://example.com/map.xml").expect("url");
    let artifact = build_artifact(
        url.clone(), url.clone(), 200,
        std::collections::BTreeMap::from([
            ("content-type".into(), "application/xml".into()),
            ("content-encoding".into(), "gzip".into()),
        ]),
        BodyStorage::inline(compressed), Vec::new(), TransportProvenance::Test, None,
    );
    assert_eq!(sitemap_body(&artifact).expect("decode"), b"<urlset/>");

    let mut unknown = artifact;
    unknown.response_headers.insert("content-encoding".into(), "br".into());
    assert!(sitemap_body(&unknown).is_err());
}
