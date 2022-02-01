// gate: G-ARCH-2
// fixture: arch.wasm.capability_fixture

use pandark_wasm::{
    extract_artifact_json_str, inspect_artifact_json_str, plan_crawl_json_str, pandark_version,
};

#[test]
fn g_arch_wasm_capability_fixture() {
    assert!(!pandark_version().is_empty());

    let artifact_json = r#"{
        "request_url": "https://example.com/",
        "final_url": "https://example.com/",
        "status": 200,
        "response_headers": { "content-type": "text/html" },
        "content_type": "text/html",
        "body": { "inline": [60,104,116,109,108,62,60,97,32,104,114,101,102,61,34,47,110,101,120,116,34,62,108,105,110,107,60,47,97,62,60,47,104,116,109,108,62] },
        "redirect_chain": [],
        "retrieved_at_epoch": 1,
        "transport_provenance": "test",
        "cache_key": "https://example.com/"
    }"#;

    let extract_json =
        extract_artifact_json_str(artifact_json).expect("extract_artifact_json_str");
    assert!(extract_json.contains("\"operation\":\"extract\""));
    assert!(extract_json.contains("\"status\":\"complete\""));

    let inspect_json =
        inspect_artifact_json_str(artifact_json, "links").expect("inspect_artifact_json_str");
    assert!(inspect_json.contains("\"discovered_links\""));
    assert!(inspect_json.contains("/next"));

    let plan_json = plan_crawl_json_str("https://example.com/", Some(2), Some(100))
        .expect("plan_crawl_json_str");
    assert!(plan_json.contains("\"schema_version\":\"pandark.plan/v1\""));
    assert!(plan_json.contains("https://example.com/"));
}
