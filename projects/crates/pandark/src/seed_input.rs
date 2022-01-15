use std::fs;
use std::path::{Path, PathBuf};

use pandark_types::{CrawlError, Result};
use url::Url;

/// Resolve crawl seed CLI input into one or more URL strings.
///
/// HTTP/HTTPS/`file://` inputs are treated as a single seed. Existing regular
/// files are read as newline-delimited seed lists (`#` comments and blank lines
/// are ignored). Other paths are normalized to a `file://` URL for a single
/// local seed.
pub fn resolve_crawl_seed_specs(input: &str) -> Result<Vec<String>> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(CrawlError::InvalidInput("crawl seed input is empty".into()));
    }
    if looks_like_url(trimmed) {
        return Ok(vec![trimmed.to_string()]);
    }
    let path = Path::new(trimmed);
    if path.is_file() {
        return read_seed_file(path);
    }
    Ok(vec![normalize_local_or_url_seed(trimmed)?])
}

fn looks_like_url(value: &str) -> bool {
    value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("file://")
}

fn read_seed_file(path: &Path) -> Result<Vec<String>> {
    let content = fs::read_to_string(path).map_err(|error| {
        CrawlError::InvalidInput(format!("read seed file `{}`: {error}", path.display()))
    })?;
    let mut seeds = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let seed = if looks_like_url(trimmed) {
            trimmed.to_string()
        } else {
            normalize_local_or_url_seed(trimmed).map_err(|error| {
                CrawlError::InvalidInput(format!(
                    "seed file `{}` line {}: {error}",
                    path.display(),
                    index + 1
                ))
            })?
        };
        seeds.push(seed);
    }
    if seeds.is_empty() {
        return Err(CrawlError::InvalidInput(format!(
            "seed file `{}` contains no usable seeds",
            path.display()
        )));
    }
    Ok(seeds)
}

fn normalize_local_or_url_seed(seed: &str) -> Result<String> {
    if looks_like_url(seed) {
        return Ok(seed.to_string());
    }
    let path = PathBuf::from(seed);
    let url = Url::from_file_path(path).map_err(|_| {
        CrawlError::InvalidInput(format!("invalid crawl seed path or URL `{seed}`"))
    })?;
    Ok(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_seed_file_with_comments() {
        let dir = std::env::temp_dir().join(format!("pandark-seeds-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("seeds.txt");
        std::fs::write(
            &path,
            "# docs\nhttps://example.com/a\n\nhttps://example.com/b\n",
        )
        .expect("write");
        let seeds = resolve_crawl_seed_specs(path.to_str().expect("utf8")).expect("seeds");
        assert_eq!(seeds.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn single_url_input_is_not_treated_as_file() {
        let seeds = resolve_crawl_seed_specs("https://example.com/only").expect("seed");
        assert_eq!(seeds, vec!["https://example.com/only".to_string()]);
    }
}
