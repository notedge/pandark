// gate: G-ARCH-4
// fixture: arch.wasm.no_native_import
//
// Compile the adapter for `wasm32-unknown-unknown` and inspect the linked
// artifact: the module must import nothing at all. A filesystem
// (`wasi_snapshot_preview1`), a native browser, an embedded YYDB or bindgen
// host glue would show up in the import section and fail the gate.

use std::{env, path::PathBuf, process::Command};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).expect("workspace root").to_path_buf()
}

fn artifact_path(root: &PathBuf) -> PathBuf {
    let target_dir = env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| root.join("target"));
    target_dir.join("wasm32-unknown-unknown").join("release").join("pandark_wasm.wasm")
}

fn build_wasm(root: &PathBuf) {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(root)
        .args(["build", "-p", "pandark-wasm", "--target", "wasm32-unknown-unknown", "--release"])
        .status()
        .expect("run `cargo` to build the wasm artifact");
    assert!(
        status.success(),
        "the `wasm32-unknown-unknown` build failed. Install the target with `rustup target add wasm32-unknown-unknown`"
    );
}

fn read_u32(bytes: &[u8], pos: &mut usize) -> u32 {
    let mut result = 0u32;
    let mut shift = 0;
    loop {
        let byte = bytes[*pos];
        *pos += 1;
        result |= u32::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return result;
        }
        shift += 7;
    }
}

fn read_name(bytes: &[u8], pos: &mut usize) -> String {
    let len = read_u32(bytes, pos) as usize;
    let value = String::from_utf8(bytes[*pos..*pos + len].to_vec()).expect("wasm names are UTF-8");
    *pos += len;
    value
}

fn skip_limits(bytes: &[u8], pos: &mut usize) {
    let flags = bytes[*pos];
    *pos += 1;
    read_u32(bytes, pos);
    if flags & 1 != 0 {
        read_u32(bytes, pos);
    }
}

fn imports(bytes: &[u8]) -> Vec<(String, String)> {
    let mut pos = 8;
    let mut found = Vec::new();
    while pos < bytes.len() {
        let id = bytes[pos];
        pos += 1;
        let size = read_u32(bytes, &mut pos) as usize;
        let end = pos + size;
        if id == 2 {
            let count = read_u32(bytes, &mut pos);
            for _ in 0..count {
                let module = read_name(bytes, &mut pos);
                let field = read_name(bytes, &mut pos);
                match bytes[pos] {
                    0 => {
                        pos += 1;
                        read_u32(bytes, &mut pos);
                    }
                    1 => {
                        pos += 1;
                        skip_limits(bytes, &mut pos);
                    }
                    2 => {
                        pos += 1;
                        skip_limits(bytes, &mut pos);
                    }
                    3 => {
                        pos += 1;
                        pos += 1;
                    }
                    kind => panic!("unsupported wasm import kind `{kind}`"),
                }
                found.push((module, field));
            }
        }
        pos = end;
    }
    found
}

fn exports(bytes: &[u8]) -> Vec<String> {
    let mut pos = 8;
    let mut found = Vec::new();
    while pos < bytes.len() {
        let id = bytes[pos];
        pos += 1;
        let size = read_u32(bytes, &mut pos) as usize;
        let end = pos + size;
        if id == 7 {
            let count = read_u32(bytes, &mut pos);
            for _ in 0..count {
                found.push(read_name(bytes, &mut pos));
                pos += 1;
                read_u32(bytes, &mut pos);
            }
        }
        pos = end;
    }
    found
}

#[test]
fn g_arch_wasm_no_native_import() {
    let root = workspace_root();
    build_wasm(&root);
    let path = artifact_path(&root);
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("read `{}`: {error}", path.display()));
    assert_eq!(&bytes[..4], b"\0asm", "the artifact must be a wasm module");

    let imports = imports(&bytes);
    assert!(
        imports.is_empty(),
        "the wasm artifact must not import anything. G-ARCH-4 forbids native filesystem, browser, YYDB and bindgen glue imports. all imports: {imports:?}"
    );

    let exports = exports(&bytes);
    for required in [
        "memory",
        "pandark_wasm_abi_version",
        "pandark_wasm_alloc",
        "pandark_wasm_dealloc",
        "pandark_wasm_run",
        "pandark_wasm_result_ptr",
        "pandark_wasm_result_len",
        "pandark_wasm_version_ptr",
        "pandark_wasm_version_len",
    ] {
        assert!(exports.iter().any(|name| name == required), "the artifact must export `{required}`, exports: {exports:?}");
    }
}
