use std::path::Path;
use tokio::fs::File;

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
