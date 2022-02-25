use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use notedown_formats::export::markdown_project::{MarkdownProject, export_markdown_project};
use notedown_ir::DocumentGraph;
use pandark_types::{CrawlError, ExtractResult, Result};

/// Published Markdown project layout for a single extracted page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedPageProject {
    /// Output directory root.
    pub output_dir: PathBuf,
    /// Written `index.md` path.
    pub index_path: PathBuf,
    /// Written asset paths relative to `output_dir`.
    pub asset_paths: Vec<String>,
    /// Written chapter paths relative to `output_dir`.
    pub chapter_paths: Vec<String>,
    /// Written `panduck.report.json` path for the page export manifest.
    pub report_path: PathBuf,
}

/// Writes a single extracted HTML page to the shared Markdown project directory contract.
pub fn publish_extracted_page(
    output_dir: impl AsRef<Path>,
    label: &str,
    extract: &ExtractResult,
) -> Result<PublishedPageProject> {
    let document =
        extract.document.as_ref().ok_or_else(|| CrawlError::InvalidInput("missing document graph for page publish".into()))?;
    let project = export_markdown_project(document).map_err(map_format_error)?;
    let report_json = build_page_report_json(label, extract, document, &project);
    publish_markdown_project(output_dir.as_ref(), &project, &report_json)
}

fn build_page_report_json(label: &str, extract: &ExtractResult, graph: &DocumentGraph, project: &MarkdownProject) -> String {
    let published_assets: Vec<String> = project.assets.iter().map(|asset| asset.relative_path.clone()).collect();
    let published_chapters: Vec<String> = project.chapters.iter().map(|chapter| chapter.relative_path.clone()).collect();
    let mut outputs = vec![
        serde_json::json!({ "path": "index.md", "format": "markdown", "published": true }),
        serde_json::json!({
            "path": "assets/",
            "format": "assets",
            "published_asset_count": published_assets.len()
        }),
    ];
    for chapter in &published_chapters {
        outputs.push(serde_json::json!({ "path": chapter, "format": "markdown", "published": true }));
    }
    outputs.push(serde_json::json!({ "path": "panduck.report.json", "format": "report", "published": true }));
    serde_json::json!({
        "schema_version": "panduck.report/v1",
        "operation": "extract_page",
        "status": if graph.coverage.complete { "success" } else { "success_with_loss" },
        "inputs": [{ "path": label, "format": "html", "source_url": extract.source_url.to_string() }],
        "pipeline": {
            "reader": extract.extractor,
            "writer": "markdown-project",
            "stages": ["fetch", "extract", "ir", "write", "materialize"]
        },
        "coverage": graph.coverage,
        "losses": graph.coverage.loss,
        "outputs": outputs,
        "unresolved_assets": project.unresolved_asset_sources,
    })
    .to_string()
}

fn publish_markdown_project(output_dir: &Path, project: &MarkdownProject, report_json: &str) -> Result<PublishedPageProject> {
    fs::create_dir_all(output_dir).map_err(map_io_error)?;
    let assets_dir = output_dir.join("assets");
    fs::create_dir_all(&assets_dir).map_err(map_io_error)?;

    let index_path = output_dir.join("index.md");
    publish_text(&index_path, &project.index_markdown)?;

    let mut chapter_paths = Vec::with_capacity(project.chapters.len());
    for chapter in &project.chapters {
        let chapter_path = output_dir.join(&chapter.relative_path);
        if let Some(parent) = chapter_path.parent() {
            fs::create_dir_all(parent).map_err(map_io_error)?;
        }
        publish_text(&chapter_path, &chapter.markdown)?;
        chapter_paths.push(chapter.relative_path.clone());
    }

    let mut asset_paths = Vec::with_capacity(project.assets.len());
    for asset in &project.assets {
        let file_name = asset
            .relative_path
            .strip_prefix("assets/")
            .ok_or_else(|| CrawlError::InvalidInput(format!("asset path must start with assets/: {}", asset.relative_path)))?;
        let path = assets_dir.join(file_name);
        publish_bytes(&path, &asset.bytes)?;
        asset_paths.push(asset.relative_path.clone());
    }

    let report_path = output_dir.join("panduck.report.json");
    publish_text(&report_path, report_json)?;

    Ok(PublishedPageProject { output_dir: output_dir.to_path_buf(), index_path, chapter_paths, asset_paths, report_path })
}

fn publish_text(path: &Path, content: &str) -> Result<()> {
    publish_bytes(path, content.as_bytes())
}

fn publish_bytes(path: &Path, content: &[u8]) -> Result<()> {
    publish_bytes_atomic(path, content).map_err(map_io_error)
}

fn publish_bytes_atomic(path: &Path, content: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "destination has no parent directory"))?;
    if !parent.is_dir() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "destination parent directory does not exist"));
    }

    let staging_path = staging_path(parent, path)?;
    {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&staging_path)?;
        file.write_all(content)?;
        file.sync_all()?;
    }

    replace_path(&staging_path, path)?;
    sync_parent(path)?;
    Ok(())
}

fn staging_path(parent: &Path, target: &Path) -> io::Result<PathBuf> {
    let file_name =
        target.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "destination has no file name"))?;
    let nanos =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|duration| duration.as_nanos()).unwrap_or(0);
    Ok(parent.join(format!(".{}.{}.{}.tmp", file_name.to_string_lossy(), std::process::id(), nanos)))
}

fn replace_path(from: &Path, to: &Path) -> io::Result<()> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            fs::remove_file(to)?;
            fs::rename(from, to)
        }
        Err(error) => Err(error),
    }
}

fn sync_parent(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "destination has no parent directory"))?;
    if let Ok(dir) = File::open(parent) {
        let _ = dir.sync_all();
    }
    Ok(())
}

fn map_format_error(error: notedown_formats::FormatError) -> CrawlError {
    CrawlError::InvalidInput(error.to_string())
}

fn map_io_error(error: io::Error) -> CrawlError {
    CrawlError::InvalidInput(error.to_string())
}
