use async_zip::tokio::write::ZipFileWriter;
use async_zip::{Compression, ZipEntryBuilder};
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

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
        let file = File::open(file_path).await?;

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

    for (name, reader) in entries {
        let builder = ZipEntryBuilder::new(name.as_ref().to_string().into(), Compression::Deflate);

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
        .and_then(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0))
        .and_then(|dt| Some(async_zip::ZipDateTime::from_chrono(&dt)))
        .unwrap_or_else(async_zip::ZipDateTime::default)
}
