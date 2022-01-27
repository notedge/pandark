// gate: G-ARCH-1
// fixture: arch.types.no_backend_deps

#[test]
fn g_arch_types_no_backend_deps() {
    let manifest = include_str!("../Cargo.toml");
    assert!(
        !manifest.contains("yydb"),
        "pandark-types must not depend on yydb"
    );
    assert!(
        !manifest.contains("napi"),
        "pandark-types must not depend on napi"
    );
    assert!(
        !manifest.contains("wasm-bindgen"),
        "pandark-types must not depend on wasm-bindgen"
    );
}
