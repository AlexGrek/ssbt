use anyhow::Result;
use bytes::Bytes;
use chrono::DateTime;
use futures::{Stream, StreamExt}; // Use futures::Stream
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf}; // Import Path
use streaming_zip::{Archive, CompressionMode};
use tokio::sync::mpsc;
use tokio::task;
use tokio_stream::wrappers::ReceiverStream;

use crate::packaging::FileEntry;

/// Compression algorithm to use when creating the ZIP.
#[derive(Debug, Clone, Copy)]
pub enum Compressor {
    Deflate,
    Stored,
}

/// Defines the destination for the generated ZIP archive.
pub enum OutSink {
    /// Save the archive to a local file at the given path.
    SaveToFile(PathBuf),
    /// Upload the archive to a remote URL via HTTP POST.
    UploadToUrl(String),
}

// --- The Core Async Solution (Unchanged) ---

struct WriterToAsyncChannel {
    sender: mpsc::Sender<Result<Vec<u8>>>,
}

impl Write for WriterToAsyncChannel {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sender
            .blocking_send(Ok(buf.to_vec()))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::BrokenPipe, e.to_string()))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn package_zip_streaming_async(
    compressor: Compressor,
    files: Vec<FileEntry>,
) -> impl Stream<Item = Result<Bytes>> {
    let (tx, rx) = mpsc::channel::<Result<Vec<u8>>>(4);

    task::spawn_blocking(move || {
        let compression = match compressor {
            Compressor::Deflate => CompressionMode::Deflate(8),
            Compressor::Stored => CompressionMode::Store,
        };
        let mut pipe = WriterToAsyncChannel { sender: tx.clone() };
        let mut archive = Archive::new(&mut pipe);
        let mut buf = [0u8; 8192];

        for fe in files {
            let mut f = match File::open(&fe.path) {
                Ok(f) => f,
                Err(e) => {
                    let _ = tx.blocking_send(Err(e.into()));
                    return;
                }
            };

            if let Err(e) = archive.start_new_file(
                fe.name_in_archive.as_bytes().to_vec(),
                DateTime::from_timestamp(0, 0).unwrap().naive_local(),
                compression,
                true,
            ) {
                let _ = tx.blocking_send(Err(anyhow::anyhow!("Failed to start new file: {:?}", e)));
                return;
            }

            loop {
                match f.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
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
    });

    ReceiverStream::new(rx).map(|r| r.map(Bytes::from))
}

// --- Example Async Consumers (One small improvement) ---

pub async fn send_http_async<S>(url: &str, stream: S) -> Result<()>
where
    S: Stream<Item = Result<Bytes, anyhow::Error>> + Send + Sync + 'static,
{
    let body = reqwest::Body::wrap_stream(stream);
    let client = reqwest::Client::new();
    let resp = client.post(url).body(body).send().await?;
    println!("Async HTTP response: {}", resp.status());
    resp.error_for_status()?;
    Ok(())
}

/// Saves ZIP data to a local file asynchronously (now generic over path).
pub async fn save_file_async<S, P>(path: P, mut stream: S) -> Result<()>
where
    S: Stream<Item = Result<Bytes>> + Unpin,
    P: AsRef<Path>, // More idiomatic: accepts &str, String, PathBuf
{
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;

    let mut f = File::create(path).await?;
    while let Some(chunk_result) = stream.next().await {
        f.write_all(&chunk_result?).await?;
    }
    f.flush().await?;
    Ok(())
}
