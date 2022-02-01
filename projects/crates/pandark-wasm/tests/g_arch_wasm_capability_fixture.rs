// gate: G-ARCH-4
// fixture: arch.wasm.capability_fixture

use pandark_types::{BodyStorage, FetchArtifact, TransportProvenance};
use pandark_wasm::api::{extract_artifact_json, inspect_artifact_json, plan_crawl_json};
use url::Url;

fn sample_artifact() -> FetchArtifact {
    FetchArtifact {
        request_url: Url::parse("https://example.com/page").expect("url"),
        final_url: Url::parse("https://example.com/page").expect("url"),
        status: 200,
        response_headers: Default::default(),
        content_type: Some("text/html".into()),
        body: BodyStorage::inline(
            br#"<html><head><title>Example</title></head><body><a href="/next">next</a></body></html>"#,
        ),
        redirect_chain: Vec::new(),
        retrieved_at_epoch: 1,
        transport_provenance: TransportProvenance::Test,
        cache_key: "https://example.com/page".into(),
    }
}

#[test]
fn g_arch_wasm_capability_fixture() {
    let plan = plan_crawl_json("https://example.com/", Some(1), Some(4)).expect("plan");
    assert!(plan.contains("https://example.com/"));

    let artifact_json = serde_json::to_string(&sample_artifact()).expect("artifact json");
    let inspect = inspect_artifact_json(&artifact_json, "links").expect("inspect");
    assert!(inspect.contains("discovered_links"));

    let extract = extract_artifact_json(&artifact_json).expect("extract");
    assert!(extract.contains("\"operation\":\"extract\""));
}
