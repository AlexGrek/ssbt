use std::path::PathBuf;

pub mod save_file;
pub mod send_net;

/// Defines the destination for the generated backup archive.
pub enum OutSink {
    /// Save the archive to a local file at the given path.
    SaveToFile(PathBuf),
    /// Upload the archive to a remote URL via HTTP POST.
    UploadToUrl(String),
}
