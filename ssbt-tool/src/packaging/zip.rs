use anyhow::Result;
use bytes::Bytes;
use chrono::DateTime;
use futures::{Stream, StreamExt}; // Use futures::Stream
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use streaming_zip::{Archive, CompressionMode};
use tokio::sync::mpsc;
use tokio::task;
use tokio_stream::wrappers::ReceiverStream;

// --- Structs and Enums (Unchanged) ---

/// Represents a file to include in the ZIP archive.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name_in_archive: String,
}

/// Compression algorithm to use when creating the ZIP.
#[derive(Debug, Clone, Copy)]
pub enum Compressor {
    Deflate,
    Stored,
}

// --- The Core Async Solution ---

/// A custom `std::io::Write` implementation that sends data into a
/// `tokio::sync::mpsc::Sender` using *blocking* sends.
/// This is designed to be used *inside* `tokio::task::spawn_blocking`.
struct WriterToAsyncChannel {
    sender: mpsc::Sender<Result<Vec<u8>>>,
}

impl Write for WriterToAsyncChannel {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let data = buf.to_vec();
        // Use blocking_send to apply backpressure.
        // This will block the *current thread* (which is a blocking pool thread)
        // if the async receiver is not keeping up.
        self.sender
            .blocking_send(Ok(data))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::BrokenPipe, e.to_string()))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Creates an async streaming ZIP archive.
///
/// This function spawns a blocking task to handle the synchronous file I/O
/// and zipping logic, piping the resulting `Bytes` chunks through an async
/// channel, which provides backpressure.
pub fn package_zip_streaming_async(
    compressor: Compressor,
    files: Vec<FileEntry>,
) -> impl Stream<Item = Result<Bytes>> {
    // 1. Create the async channel. A small buffer (e.g., 4) is ideal for
    // backpressure without consuming much memory.
    let (tx, rx) = mpsc::channel::<Result<Vec<u8>>>(4);

    // 2. Spawn the synchronous, CPU-bound, and I/O-bound zipping logic
    // onto tokio's blocking thread pool.
    task::spawn_blocking(move || {
        let compression = match compressor {
            Compressor::Deflate => CompressionMode::Deflate(8),
            Compressor::Stored => CompressionMode::Store,
        };

        // This closure owns `tx`.
        // We clone it for the writer and keep `tx` for sending errors.
        let mut pipe = WriterToAsyncChannel { sender: tx.clone() };
        let mut archive = Archive::new(&mut pipe);

        let mut buf = [0u8; 8192]; // 8KB read buffer

        for fe in files {
            let mut f = match File::open(&fe.path) {
                Ok(f) => f,
                Err(e) => {
                    // Send the error and abort.
                    // We don't care if the send fails (receiver hung up).
                    let _ = tx.blocking_send(Err(e.into()));
                    return; // Stop the blocking task
                }
            };

            if let Err(e) = archive.start_new_file(
                fe.name_in_archive.as_bytes().to_vec(),
                DateTime::from_timestamp(0, 0).unwrap().naive_local(),
                compression,
                true, // Use Zip64 for large files
            ) {
                let _ = tx.blocking_send(Err(anyhow::anyhow!("Failed to start new file: {:?}", e)));
                return;
            }

            loop {
                match f.read(&mut buf) {
                    Ok(0) => break, // End of file
                    Ok(n) => {
                        // This `write` call will block if the channel is full.
                        if let Err(e) = archive.append_data(&buf[..n]) {
                            let _ = tx.blocking_send(Err(anyhow::anyhow!("Failed to append data: {:?}", e)));
                            return;
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => {
                        let _ = tx.blocking_send(Err(e.into()));
                        return;
                    }
                }
            }

            if let Err(e) = archive.finish_file() {
                let _ = tx.blocking_send(Err(anyhow::anyhow!("Failed to finish file: {:?}", e)));
                return;
            }
        }

        if let Err(e) = archive.finish() {
            let _ = tx.blocking_send(Err(anyhow::anyhow!("Failed to finish archive: {:?}", e)));
        }

        // When this closure ends, `tx` and its clone in `pipe` are dropped.
        // This closes the channel, which `ReceiverStream` interprets as
        // the end of the stream (it will yield None).
    });

    // 3. Return the stream
    // Convert the Receiver into a Stream.
    // Map the `Result<Vec<u8>>` to `Result<Bytes>`.
    ReceiverStream::new(rx).map(|r| r.map(Bytes::from))
}

// --- Example Async Consumers ---

/// Sends ZIP data to an HTTP endpoint asynchronously using reqwest.
pub async fn send_http_async<S>(url: &str, stream: S) -> Result<()>
where
    // The stream must be:
    // 1. A Stream of Result<Bytes, anyhow::Error>
    // 2. Send + Sync: Movable between threads
    // 3. 'static: Lives long enough
    S: Stream<Item = Result<Bytes, anyhow::Error>> + Send + Sync + 'static,
{
    // reqwest::Body::wrap_stream automatically handles the stream.
    // It requires the Error type to be `Into<Box<dyn Error + Send + Sync>>`,
    // which `anyhow::Error` satisfies perfectly.
    let body = reqwest::Body::wrap_stream(stream);

    let client = reqwest::Client::new();
    let resp = client.post(url)
        .body(body)
        .send()
        .await?;

    println!("Async HTTP response: {}", resp.status());
    resp.error_for_status()?; // Check for 4xx/5xx errors
    Ok(())
}

/// Saves ZIP data to a local file asynchronously using tokio.
pub async fn save_file_async<S>(path: &str, mut stream: S) -> Result<()>
where
    // Add `Unpin` bound because we are calling `stream.next()` directly.
    S: Stream<Item = Result<Bytes>> + Unpin,
{
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;

    let mut f = File::create(path).await?;
    
    // Process the stream
    while let Some(chunk_result) = stream.next().await {
        f.write_all(&chunk_result?).await?;
    }
    
    f.flush().await?;
    Ok(())
}