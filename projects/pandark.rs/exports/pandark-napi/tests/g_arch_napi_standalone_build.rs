// gate: G-ARCH-3
// fixture: arch.napi.standalone_build

#[test]
fn g_arch_napi_standalone_build() {
    let version = pandark_napi::pandark_version();
    assert!(!version.is_empty());
}
