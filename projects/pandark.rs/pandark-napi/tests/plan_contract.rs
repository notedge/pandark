//! Smoke contract for the Node-API crawl plan surface.
//!
//! Crawl and extraction behaviour coverage lives in `projects/pandark.ts/packages/pandark/tests`.
//! This test only proves planning is wired and empty seeds are rejected.

#[test]
fn plan_crawl_accepts_one_seed() {
    let plan = pandark_napi::plan_crawl("https://example.com".into(), Some(1), Some(2)).expect("plan one seed");
    assert!(!plan.report_json.is_empty());
}

#[test]
fn plan_crawl_rejects_empty_seed() {
    assert!(pandark_napi::plan_crawl(String::new(), None, None).is_err());
}
