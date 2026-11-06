// gate: G-ARCH-3
// fixture: arch.napi.standalone_build
//
// The gate must prove the crate really produces a loadable Node-API binary and
// that the binary is self-contained: the compiled cdylib is copied out as a
// `.node` file into an otherwise empty directory and loaded by a fresh `node`
// process. A version-string assertion alone would pass even if the cdylib were
// broken or accidentally linked against private Pandark modules.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn cdylib_file_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "pandark_napi.dll"
    }
    else if cfg!(target_os = "macos") {
        "libpandark_napi.dylib"
    }
    else {
        "libpandark_napi.so"
    }
}

/// Locate the cdylib that `cargo test` built for this package.
///
/// Cargo may place the test binary and the library in different artifact
/// directories: `target/<profile>/deps`, a build-script output directory, or
/// `target/<profile>/build/pandark-napi/<hash>/out`. Every candidate is
/// collected and the newest one is used, so the gate never loads a stale
/// library left behind by an earlier layout or an earlier build.
fn locate_cdylib() -> PathBuf {
    let exe = std::env::current_exe().expect("test binary path");
    let file_name = cdylib_file_name();
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = exe.parent() {
        candidates.push(dir.join(file_name));
    }
    let components: Vec<_> = exe.components().collect();
    if let Some(index) = components.iter().position(|component| component.as_os_str() == "target") {
        if let Some(profile) = components.get(index + 1) {
            let mut target_dir = PathBuf::new();
            for component in &components[..=index] {
                target_dir.push(component.as_os_str());
            }
            let profile_dir = target_dir.join(profile.as_os_str());
            candidates.push(profile_dir.join("deps").join(file_name));
            if let Ok(entries) = std::fs::read_dir(profile_dir.join("build").join("pandark-napi")) {
                for entry in entries.flatten() {
                    candidates.push(entry.path().join("out").join(file_name));
                }
            }
        }
    }
    let newest = candidates
        .into_iter()
        .filter(|candidate| candidate.is_file())
        .filter_map(|candidate| {
            let metadata = std::fs::metadata(&candidate).ok()?;
            if metadata.len() == 0 {
                return None;
            }
            Some((metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH), candidate))
        })
        .max_by_key(|(modified, _)| *modified);
    match newest {
        Some((_, path)) => path,
        None => panic!("the gate needs the cdylib built from `crate-type = [\"cdylib\", \"rlib\"]` under `target/<profile>`"),
    }
}

struct NodeArtifact {
    dir: PathBuf,
    file: PathBuf,
}

impl NodeArtifact {
    fn install(cdylib: &Path) -> Self {
        let dir = std::env::temp_dir().join(format!("pandark-napi-gate-{}", std::process::id()));
        // Leave nothing behind from a previous aborted run.
        let _ = std::fs::remove_file(dir.join("pandark.napi.node"));
        let _ = std::fs::remove_dir(&dir);
        std::fs::create_dir(&dir).expect("create the gate directory");
        let file = dir.join("pandark.napi.node");
        std::fs::copy(cdylib, &file).expect("copy the cdylib as a .node file");
        Self { dir, file }
    }
}

impl Drop for NodeArtifact {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.file);
        let _ = std::fs::remove_dir(&self.dir);
    }
}

fn load_through_node(artifact: &NodeArtifact) -> String {
    let script = r#"
const { pandarkVersion } = require(process.env.PANDARK_GATE_MODULE);
const version = pandarkVersion();
if (typeof version !== "string" || version.length === 0) {
    console.error("the standalone .node returned an empty version");
    process.exit(2);
}
process.stdout.write(version);
"#;
    let output = Command::new("node")
        .arg("--eval")
        .arg(script)
        .env("PANDARK_GATE_MODULE", &artifact.file)
        .current_dir(&artifact.dir)
        .output()
        .expect("run `node` to load the standalone .node. The gate requires a Node runtime on PATH");
    assert!(output.status.success(), "node failed to load the standalone .node: {}", String::from_utf8_lossy(&output.stderr));
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert!(!version.is_empty(), "the standalone .node loaded but returned an empty version");
    version
}

#[test]
fn g_arch_napi_standalone_build() {
    let cdylib = locate_cdylib();
    let artifact = NodeArtifact::install(&cdylib);
    let version = load_through_node(&artifact);
    assert_eq!(version, env!("CARGO_PKG_VERSION"), "the loaded .node must report the crate version");
    assert!(pandark_napi::pandark_version() == version, "the in-process binding and the .node must agree");
}
