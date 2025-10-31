use anyhow::Result;
use std::path::PathBuf;
use tokio::runtime::Builder;
pub mod tar;
pub mod zip;

/// Represents a file to include in the ZIP archive.
/// Added Clone so it can be moved into the async block.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name_in_archive: String,
}

