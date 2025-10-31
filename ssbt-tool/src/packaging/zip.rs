use anyhow::anyhow;
use async_zip::tokio::write::ZipFileWriter;
use async_zip::{Compression, ZipEntryBuilder};
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio_util::compat::{TokioAsyncWriteCompatExt, TokioAsyncReadCompatExt};
use futures::io::AsyncWriteExt as FuturesAsyncWriteExt;
use std::path::{Path, PathBuf};

/// Streams files into a zip archive without buffering the entire zip in memory.
/// 
/// # Arguments
/// * `files` - Iterator of (archive_path, file_path) tuples
/// * `output` - Any async writer (file, network stream, stdout, etc.)
/// 
/// # Example
/// ```no_run
/// use tokio::fs::File;
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let files = vec![
///         ("document.txt", "/path/to/file1.txt"),
///         ("folder/image.png", "/path/to/file2.png"),
///     ];
///     
///     // Stream to file
///     let output = File::create("archive.zip").await?;
///     stream_zip_to_writer(files, output).await?;
///     
///     // Or stream to HTTP response, S3, etc.
///     Ok(())
/// }
/// ```
pub async fn stream_zip_to_writer<W, I, S1, S2>(
    files: I,
    output: W,
) -> Result<(), Box<dyn std::error::Error>>
where
    W: AsyncWrite + Unpin,
    I: IntoIterator<Item = (S1, S2)>,
    S1: AsRef<str>,
    S2: AsRef<Path>,
{
    // Wrap with compat for async-zip which uses futures::io traits
    let mut writer = ZipFileWriter::new(output.compat_write());
    
    for (archive_name, file_path) in files {
        let file_path = file_path.as_ref();
        let mut file = File::open(file_path).await?;
        
        // Get file metadata for proper zip entry
        let metadata = tokio::fs::metadata(file_path).await?;
        
        let builder = ZipEntryBuilder::new(
            archive_name.as_ref().to_string().into(),
            Compression::Deflate,
        )
        .last_modification_date(get_modification_time(&metadata));
        
        // Stream file directly into zip entry with small buffer
        let mut entry_writer = writer.write_entry_stream(builder).await?;
        
        // Use futures::io::copy since entry_writer uses futures traits
        futures::io::copy(&mut file.compat(), &mut entry_writer).await?;
        entry_writer.close().await?;
    }
    
    // Finalize zip (writes central directory)
    writer.close().await?;
    
    Ok(())
}

/// Alternative: Stream from async readers instead of file paths
pub async fn stream_zip_from_readers<W, I, R, S>(
    entries: I,
    output: W,
) -> Result<(), Box<dyn std::error::Error>>
where
    W: AsyncWrite + Unpin,
    I: IntoIterator<Item = (S, R)>,
    R: AsyncRead + Unpin,
    S: AsRef<str>,
{
    let mut writer = ZipFileWriter::new(output.compat_write());
    
    for (name, mut reader) in entries {
        let builder = ZipEntryBuilder::new(
            name.as_ref().to_string().into(),
            Compression::Deflate,
        );
        
        let mut entry_writer = writer.write_entry_stream(builder).await?;
        futures::io::copy(&mut reader.compat(), &mut entry_writer).await?;
        entry_writer.close().await?;
    }
    
    writer.close().await?;
    Ok(())
}

fn get_modification_time(metadata: &std::fs::Metadata) -> async_zip::ZipDateTime {
    use std::time::SystemTime;
    
    metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .and_then(|d| {
            chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
        })
        .and_then(|dt| {
            Some(async_zip::ZipDateTime::from_chrono(&dt))
        })
        .unwrap_or_else(async_zip::ZipDateTime::default)
}

// Example usage with different outputs
#[cfg(test)]
mod examples {
    use super::*;
    use tokio::io::AsyncWriteExt;
    
    // Stream to file
    async fn example_to_file() -> Result<(), Box<dyn std::error::Error>> {
        let files = vec![
            ("readme.txt", "/tmp/readme.txt"),
            ("data/config.json", "/tmp/config.json"),
        ];
        
        let output = File::create("output.zip").await?;
        stream_zip_to_writer(files, output).await?;
        Ok(())
    }
    
    // Stream to network (e.g., HTTP response)
    async fn example_to_network() -> Result<(), Box<dyn std::error::Error>> {
        let files = vec![
            ("file1.txt", "/tmp/file1.txt"),
        ];
        
        // In actix-web or axum, this could be the response body stream
        let tcp_stream = tokio::net::TcpStream::connect("127.0.0.1:8080").await?;
        stream_zip_to_writer(files, tcp_stream).await?;
        Ok(())
    }
    
    // Stream to S3 or cloud storage
    async fn example_to_buffer() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let files = vec![
            ("test.txt", "/tmp/test.txt"),
        ];
        
        let mut buffer = Vec::new();
        stream_zip_to_writer(files, &mut buffer).await?;
        Ok(buffer)
    }
}

/// Creates a file writer for streaming zip output.
/// Automatically creates parent directories if they don't exist.
/// 
/// # Example
/// ```no_run
/// use tokio::fs::File;
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let files = vec![
///         ("document.txt", "/path/to/file1.txt"),
///         ("image.png", "/path/to/file2.png"),
///     ];
///     
///     let writer = create_file_writer("output/archive.zip").await?;
///     stream_zip_to_writer(files, writer).await?;
///     Ok(())
/// }
/// ```
pub async fn create_file_writer<P: AsRef<Path>>(
    path: P,
) -> Result<File, Box<dyn std::error::Error>> {
    let path = path.as_ref();
    
    // Create parent directories if they don't exist
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    
    // Create the file
    let file = File::create(path).await?;
    
    Ok(file)
}

/// Defines where the zip archive should be sent.
#[derive(Debug, Clone)]
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
            let writer = create_file_writer(path).await?;
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
                        tokio_util::io::ReaderStream::new(reader)
                    ))
                    .send()
                    .await?;
                
                if !response.status().is_success() {
                    return Err(format!(
                        "Upload failed with status: {}",
                        response.status()
                    ).into());
                }
                
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
            });
            
            // Stream zip to the writer end
            stream_zip_to_writer(files, writer).await?;
            
            // Wait for upload to complete and convert the error
            upload_task.await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?.map_err(|e| anyhow!(e))?;
        }
    }
    
    Ok(())
}

// Cargo.toml dependencies:
// [dependencies]
// async-zip = { version = "0.0.17", features = ["tokio", "deflate"] }
// tokio = { version = "1", features = ["fs", "io-util", "rt", "macros"] }
// tokio-util = { version = "0.7", features = ["io", "compat"] }
// futures = "0.3"
// reqwest = { version = "0.12", features = ["stream"] }
// chrono = "0.4"