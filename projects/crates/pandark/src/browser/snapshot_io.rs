use std::fs;
use std::path::Path;

use pandark_types::{BrowserSnapshot, CrawlError, Result};

/// Load a `BrowserSnapshot` from JSON on disk.
pub fn load_snapshot_json(path: impl AsRef<Path>) -> Result<BrowserSnapshot> {
    let bytes = fs::read(path.as_ref()).map_err(|error| {
        CrawlError::InvalidInput(format!("read snapshot `{}`: {error}", path.as_ref().display()))
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        CrawlError::InvalidInput(format!("parse snapshot JSON: {error}"))
    })
}

/// Detect whether a path likely contains a browser snapshot.
pub fn path_looks_like_snapshot(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("json") || ext.eq_ignore_ascii_case("snapshot"))
        .unwrap_or(false)
}
