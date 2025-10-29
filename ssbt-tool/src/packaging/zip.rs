use anyhow::Result;
use bytes::Bytes;
use chrono::DateTime;
use futures::{Stream, StreamExt};
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use streaming_zip::{Archive, CompressionMode};
use std::sync::mpsc;
use std::thread;

/// Represents a file to include in the ZIP archive.
pub struct FileEntry {
    pub path: PathBuf,
    pub name_in_archive: String,
}

/// Compression algorithm to use when creating the ZIP.
pub enum Compressor {
    Deflate,
    Stored,
}

/// Creates a streaming ZIP archive via `streaming-zip` and returns a blocking iterator of `Bytes`.
pub fn package_zip_streaming(
    compressor: Compressor,
    files: Vec<FileEntry>,
) -> Result<impl Iterator<Item = Result<Bytes>>> {
    let (tx, rx) = mpsc::sync_channel::<Result<Vec<u8>>>(4);

    thread::spawn(move || {
        let compression = match compressor {
            Compressor::Deflate => CompressionMode::Deflate(8),
            Compressor::Stored => CompressionMode::Store,
        };

        let mut pipe = WriterToChannel { sender: tx.clone() };
        let mut archive = Archive::new(&mut pipe);

        for fe in files {
            let mut f = match File::open(&fe.path) {
                Ok(f) => f,
                Err(e) => {
                    let _ = tx.send(Err(e.into()));
                    return;
                }
            };

            if let Err(e) = archive.start_new_file(
                fe.name_in_archive.as_bytes().to_vec(),
                DateTime::from_timestamp(0, 0).unwrap().naive_local(),
                compression,
                true,
            ) {
                let _ = tx.send(Err(anyhow::anyhow!("{:?}", e)));
                return;
            }

            let mut buf = [0u8; 8192];
            loop {
                match f.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if let Err(e) = archive.append_data(&buf[..n]) {
                            let _ = tx.send(Err(anyhow::anyhow!("{:?}", e)));
                            return;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e.into()));
                        return;
                    }
                }
            }

            if let Err(e) = archive.finish_file() {
                let _ = tx.send(Err(anyhow::anyhow!("{:?}", e)));
                return;
            }
        }

        if let Err(e) = archive.finish() {
            let _ = tx.send(Err(anyhow::anyhow!("{:?}", e)));
        }
    });

    Ok(rx.into_iter().map(|r| r.map(Bytes::from)))
}

/// A writer that sends data into an `mpsc` channel.
struct WriterToChannel {
    sender: mpsc::SyncSender<Result<Vec<u8>>>,
}

impl Write for WriterToChannel {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let data = buf.to_vec();
        self.sender
            .send(Ok(data))
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "channel closed"))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Sends ZIP data to an HTTP or HTTPS endpoint synchronously.
pub fn send_http<I>(url: &str, mut data: I) -> Result<()>
where
    I: Iterator<Item = Result<Bytes>>,
{
    let client = reqwest::blocking::Client::new();
    let mut body: Vec<u8> = Vec::new();
    for chunk in data {
        body.extend_from_slice(&chunk?);
    }
    let resp = client.post(url).body(body).send()?;
    println!("HTTP response: {}", resp.status());
    Ok(())
}

/// Saves ZIP data to a local file synchronously.
pub fn save_file<I>(path: &str, mut data: I) -> Result<()>
where
    I: Iterator<Item = Result<Bytes>>,
{
    let mut f = File::create(path)?;
    for chunk in data {
        f.write_all(&chunk?)?;
    }
    f.flush()?;
    Ok(())
}
