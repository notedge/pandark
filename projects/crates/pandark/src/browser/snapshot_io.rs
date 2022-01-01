use std::fs;
use std::path::Path;

use pandark_types::{BrowserSnapshot, CrawlError, Result};

use super::provider::FixtureBrowserProvider;

/// Load a `BrowserSnapshot` from JSON on disk.
pub fn load_snapshot_json(path: impl AsRef<Path>) -> Result<BrowserSnapshot> {
    let bytes = fs::read(path.as_ref()).map_err(|error| {
        CrawlError::InvalidInput(format!("read snapshot `{}`: {error}", path.as_ref().display()))
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        CrawlError::InvalidInput(format!("parse snapshot JSON: {error}"))
    })
}

/// Load fixture snapshots from a directory into a browser provider.
pub fn load_fixture_provider_from_dir(dir: impl AsRef<Path>) -> Result<FixtureBrowserProvider> {
    let dir = dir.as_ref();
    if !dir.is_dir() {
        return Err(CrawlError::InvalidInput(format!(
            "browser fixtures path is not a directory `{}`",
            dir.display()
        )));
    }

    let mut provider = FixtureBrowserProvider::new();
    let mut loaded = 0u32;
    for entry in fs::read_dir(dir).map_err(|error| {
        CrawlError::InvalidInput(format!("read browser fixtures dir `{}`: {error}", dir.display()))
    })? {
        let entry = entry.map_err(|error| {
            CrawlError::InvalidInput(format!("read browser fixtures entry: {error}"))
        })?;
        let path = entry.path();
        if !path.is_file() || !path_looks_like_snapshot(&path) {
            continue;
        }
        let snapshot = load_snapshot_json(&path)?;
        provider.insert(snapshot);
        loaded += 1;
    }

    if loaded == 0 {
        return Err(CrawlError::InvalidInput(format!(
            "browser fixtures dir `{}` contains no snapshot JSON files",
            dir.display()
        )));
    }

    Ok(provider)
}

/// Detect whether a path likely contains a browser snapshot.
pub fn path_looks_like_snapshot(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("json") || ext.eq_ignore_ascii_case("snapshot"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use crate::BrowserProvider;
    use url::Url;

    use pandark_types::PageChallengeState;

    use super::*;

    #[test]
    fn load_fixture_provider_reads_snapshot_directory() {
        let dir = std::env::temp_dir().join(format!("pandark-fixtures-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("page.snapshot.json");
        std::fs::write(
            &path,
            serde_json::to_string(&BrowserSnapshot {
                requested_url: Url::parse("https://example.com/page").expect("url"),
                final_url: Url::parse("https://example.com/page").expect("url"),
                document_html: "<html><body>ok</body></html>".into(),
                captured_at_epoch: 1,
                browser_engine: "fixture".into(),
                profile_id: "test".into(),
                challenge_state: PageChallengeState::Normal,
            })
            .expect("json"),
        )
        .expect("write");

        let provider = load_fixture_provider_from_dir(&dir).expect("load fixtures");
        let snapshot = provider
            .capture_snapshot(&Url::parse("https://example.com/page").expect("url"))
            .expect("snapshot");
        assert_eq!(snapshot.profile_id, "test");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
