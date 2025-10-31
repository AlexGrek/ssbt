use std::path::{Path, PathBuf};

use crate::packaging::zip::stream_zip_to_writer;
use anyhow::anyhow;

pub mod save_file;
pub mod send_net;

/// Defines the destination for the generated backup archive.
#[derive(Debug)]
pub enum OutSink {
    /// Save the archive to a local file at the given path.
    SaveToFile(PathBuf),
    /// Upload the archive to a remote URL via HTTP POST.
    UploadToUrl(String),
}

/// Streams zip archive to the specified output sink.
///
/// # Example
/// ```no_run
/// use std::path::PathBuf;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let files = vec![
///         ("readme.txt", "/tmp/readme.txt"),
///         ("config.json", "/tmp/config.json"),
///     ];
///     
///     // Save to file
///     let sink = OutSink::SaveToFile(PathBuf::from("backups/archive.zip"));
///     stream_zip_to_sink(files.clone(), sink).await?;
///     
///     // Upload via HTTP
///     let sink = OutSink::UploadToUrl("https://api.example.com/upload".to_string());
///     stream_zip_to_sink(files, sink).await?;
///     
///     Ok(())
/// }
/// ```
pub async fn stream_zip_to_sink<I, S1, S2>(
    files: I,
    sink: OutSink,
) -> Result<(), Box<dyn std::error::Error>>
where
    I: IntoIterator<Item = (S1, S2)>,
    S1: AsRef<str>,
    S2: AsRef<Path>,
{
    match sink {
        OutSink::SaveToFile(path) => {
            let writer = save_file::create_file_writer(path).await?;
            stream_zip_to_writer(files, writer).await?;
        }
        OutSink::UploadToUrl(url) => {
            // Create a pipe: writer end for zip, reader end for HTTP
            let (writer, reader) = tokio::io::duplex(8192);

            let client = reqwest::Client::new();

            // Spawn HTTP upload task
            let upload_task = tokio::spawn(async move {
                let response = client
                    .post(&url)
                    .header("Content-Type", "application/zip")
                    .body(reqwest::Body::wrap_stream(
                        tokio_util::io::ReaderStream::new(reader),
                    ))
                    .send()
                    .await?;

                if !response.status().is_success() {
                    return Err(format!("Upload failed with status: {}", response.status()).into());
                }

                Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
            });

            // Stream zip to the writer end
            stream_zip_to_writer(files, writer).await?;

            // Wait for upload to complete and convert the error
            upload_task
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?
                .map_err(|e| anyhow!(e))?;
        }
    }

    Ok(())
}
