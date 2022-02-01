// gate: G-ARCH-2
// fixture: arch.wasm.no_native_import

use std::fs;
use std::path::PathBuf;

#[test]
fn g_arch_wasm_no_native_import() {
    let manifest = manifest_text();
    for forbidden in [
        "yydb",
        "pandark-store",
        "pandark-fetch",
        "pandark =",
        "napi",
        "ureq",
    ] {
        assert!(
            !manifest.contains(&format!("{forbidden} =")),
            "pandark-wasm must not depend on `{forbidden}`"
        );
    }

    let lib_rs = fs::read_to_string(crate_root().join("src/lib.rs")).expect("read lib.rs");
    for forbidden in ["crawl_file", "resume_crawl", "fetch_seed", "run_crawl"] {
        assert!(
            !lib_rs.contains(forbidden),
            "pandark-wasm must not export crawl surface `{forbidden}`"
        );
    }
}

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn manifest_text() -> String {
    fs::read_to_string(crate_root().join("Cargo.toml")).expect("read Cargo.toml")
}
