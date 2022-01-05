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

/// Write a browser snapshot as JSON to disk.
pub fn save_snapshot_json(path: impl AsRef<Path>, snapshot: &BrowserSnapshot) -> Result<()> {
    let json = serde_json::to_string_pretty(snapshot).map_err(|error| {
        CrawlError::InvalidInput(format!("serialize snapshot JSON: {error}"))
    })?;
    fs::write(path.as_ref(), json).map_err(|error| {
        CrawlError::InvalidInput(format!("write snapshot `{}`: {error}", path.as_ref().display()))
    })
}

/// Parse comma-separated browser fixture directories.
pub fn parse_browser_dirs(spec: &str) -> Vec<std::path::PathBuf> {
    spec.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(std::path::PathBuf::from)
        .collect()
}

fn load_snapshots_from_dir_into(
    provider: &mut FixtureBrowserProvider,
    dir: &Path,
) -> Result<u32> {
    if !dir.is_dir() {
        return Err(CrawlError::InvalidInput(format!(
            "browser fixtures path is not a directory `{}`",
            dir.display()
        )));
    }

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
    Ok(loaded)
}

/// Load fixture snapshots from multiple directories.
///
/// Later directories override earlier snapshots for the same URL.
pub fn load_fixture_provider_from_dirs(dirs: &[impl AsRef<Path>]) -> Result<FixtureBrowserProvider> {
    if dirs.is_empty() {
        return Err(CrawlError::InvalidInput(
            "at least one browser fixtures directory is required".into(),
        ));
    }
    let mut provider = FixtureBrowserProvider::new();
    let mut loaded = 0u32;
    for dir in dirs {
        loaded += load_snapshots_from_dir_into(&mut provider, dir.as_ref())?;
    }
    if loaded == 0 {
        return Err(CrawlError::InvalidInput(
            "browser fixtures directories contain no snapshot JSON files".into(),
        ));
    }
    Ok(provider)
}

/// Load fixture snapshots from a directory into a browser provider.
pub fn load_fixture_provider_from_dir(dir: impl AsRef<Path>) -> Result<FixtureBrowserProvider> {
    let mut provider = FixtureBrowserProvider::new();
    let loaded = load_snapshots_from_dir_into(&mut provider, dir.as_ref())?;
    if loaded == 0 {
        return Err(CrawlError::InvalidInput(format!(
            "browser fixtures dir `{}` contains no snapshot JSON files",
            dir.as_ref().display()
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

    #[test]
    fn load_fixture_provider_from_dirs_later_dir_overrides_url() {
        let base = std::env::temp_dir().join(format!("pandark-fixtures-multi-{}", std::process::id()));
        let first = base.join("first");
        let second = base.join("second");
        std::fs::create_dir_all(&first).expect("first dir");
        std::fs::create_dir_all(&second).expect("second dir");
        let url = Url::parse("https://example.com/page").expect("url");
        let first_snapshot = BrowserSnapshot {
            requested_url: url.clone(),
            final_url: url.clone(),
            document_html: "<html><body>first</body></html>".into(),
            captured_at_epoch: 1,
            browser_engine: "fixture".into(),
            profile_id: "first".into(),
            challenge_state: PageChallengeState::Normal,
        };
        let second_snapshot = BrowserSnapshot {
            requested_url: url.clone(),
            final_url: url.clone(),
            document_html: "<html><body>second</body></html>".into(),
            captured_at_epoch: 2,
            browser_engine: "fixture".into(),
            profile_id: "second".into(),
            challenge_state: PageChallengeState::Normal,
        };
        save_snapshot_json(first.join("page.snapshot.json"), &first_snapshot).expect("write first");
        save_snapshot_json(second.join("page.snapshot.json"), &second_snapshot).expect("write second");

        let provider = load_fixture_provider_from_dirs(&[first, second]).expect("load fixtures");
        let snapshot = provider.capture_snapshot(&url).expect("snapshot");
        assert_eq!(snapshot.profile_id, "second");
        let _ = std::fs::remove_dir_all(&base);
    }
}
