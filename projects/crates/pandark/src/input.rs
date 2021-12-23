use std::path::Path;

use pandark_types::{BrowserSnapshot, CrawlError, FetchArtifact, Result};

use crate::browser::{load_snapshot_json, path_looks_like_snapshot};
use crate::fetch::{FetchRequest, FileTransport, Transport};

/// Input format selector for extract and inspect commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    /// Local HTML or fetch artifact bytes via file transport.
    Html,
    /// JSON browser snapshot.
    Snapshot,
    /// Infer from file extension and content.
    Auto,
}

/// Loaded CLI input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadedInput {
    /// Fetch artifact from HTML or binary file.
    Artifact(FetchArtifact),
    /// Browser snapshot JSON.
    Snapshot(BrowserSnapshot),
}

/// Parse an input format string from CLI or N-API.
pub fn parse_input_format(from: Option<&str>) -> Result<InputFormat> {
    match from.unwrap_or("auto") {
        "html" => Ok(InputFormat::Html),
        "snapshot" => Ok(InputFormat::Snapshot),
        "auto" => Ok(InputFormat::Auto),
        other => Err(CrawlError::InvalidInput(format!("unsupported input format `{other}`"))),
    }
}

/// Load extract or inspect input from a local path.
pub fn load_input(path: &str, format: InputFormat) -> Result<LoadedInput> {
    let file_path = Path::new(path);
    let resolved = match format {
        InputFormat::Html => InputFormat::Html,
        InputFormat::Snapshot => InputFormat::Snapshot,
        InputFormat::Auto => {
            if path_looks_like_snapshot(file_path) {
                InputFormat::Snapshot
            } else {
                InputFormat::Html
            }
        }
    };

    match resolved {
        InputFormat::Snapshot => Ok(LoadedInput::Snapshot(load_snapshot_json(file_path)?)),
        InputFormat::Html => {
            let file_url = url::Url::from_file_path(file_path).map_err(|_| {
                CrawlError::InvalidInput(format!("invalid input path `{path}`"))
            })?;
            let transport = FileTransport::new();
            let artifact = transport.fetch(&FetchRequest {
                url: file_url,
                headers: Default::default(),
            })?;
            Ok(LoadedInput::Artifact(artifact))
        }
        InputFormat::Auto => unreachable!("auto resolved above"),
    }
}
