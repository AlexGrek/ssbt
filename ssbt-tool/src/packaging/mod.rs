use std::path::PathBuf;
use anyhow::Result;
use tokio::runtime::Builder;

use crate::packaging::zip::{Compressor, OutSink, package_zip_streaming_async, save_file_async, send_http_async};

pub mod zip;
pub mod tar;

/// Represents a file to include in the ZIP archive.
/// Added Clone so it can be moved into the async block.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name_in_archive: String,
}

/// Creates a ZIP stream and sends it to a sink, managing its own async runtime.
///
/// This is the main entrypoint for synchronous applications.
pub fn create_zip_and_send_sync(
    compressor: Compressor,
    files: Vec<FileEntry>,
    sink: OutSink,
) -> Result<()> {
    // 1. Build a tokio runtime.
    // `new_multi_thread` will default to the number of logical CPUs.
    // `enable_all` is required for I/O and timers.
    let rt = Builder::new_multi_thread()
        .enable_all()
        .build()?;

    // 2. Run the async logic to completion on the runtime.
    // `block_on` will block the current thread until the async
    // block inside it finishes.
    rt.block_on(async {
        // Get the async stream of ZIP data
        let zip_stream = package_zip_streaming_async(compressor, files);

        // Match on the sink to determine where to send the stream
        match sink {
            OutSink::SaveToFile(path) => {
                // We must pin the stream to call `stream.next()` in save_file_async
                let mut pinned_stream = Box::pin(zip_stream);
                save_file_async(path, &mut pinned_stream).await
            }
            OutSink::UploadToUrl(url) => {
                // send_http_async takes ownership, so no pin is needed.
                send_http_async(&url, zip_stream).await
            }
        }
    })
    // The `?` here propagates any error from the async block.
}