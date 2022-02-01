// gate: G-ARCH-4
// fixture: arch.wasm.no_native_import

use std::fs;
use std::path::PathBuf;

#[test]
fn g_arch_wasm_no_native_import() {
    let src_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let forbidden = [
        "std::fs",
        "std::process",
        "yydb::",
        "use yydb",
        "napi::",
        "ureq::",
        "pandark_store",
        "pandark-store",
        "crawl_file",
        "resume_crawl",
        "run_crawl",
    ];
    for entry in fs::read_dir(src_dir).expect("read src") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("read source");
        for pattern in forbidden {
            assert!(
                !text.contains(pattern),
                "{} must not reference `{}`",
                path.display(),
                pattern
            );
        }
    }
}
