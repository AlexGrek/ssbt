use std::path::{Path, PathBuf};

use crate::packaging::zip::stream_zip_to_writer;
use anyhow::anyhow;
use async_zip::Compression;

pub mod save_file;
pub mod send_net;
pub mod send_s3;

/// Defines the destination for the generated backup archive.
#[derive(Debug)]
pub enum OutSink {
    /// Save the archive to a local file at the given path.
    SaveToFile(PathBuf),
    /// Upload the archive to a remote URL via HTTP POST.
    UploadToUrl(String),
    /// Upload the archive via FTP or FTPS.
    UploadViaFtp {
        host: String,
        port: u16,
        remote_path: String,
        user: String,
        password: String,
        secure: bool,
    },
    /// Upload the archive to an S3-compatible bucket.
    UploadToS3 {
        bucket: String,
        key: String,
        region: String,
        endpoint: Option<String>,
        access_key: Option<String>,
        secret_key: Option<String>,
    },
}

/// Streams zip archive to the specified output sink.
pub async fn stream_zip_to_sink<I, S1, S2>(
    files: I,
    compression: Compression,
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
            stream_zip_to_writer(files, compression, writer).await?;
        }
        OutSink::UploadToUrl(url) => {
            let (writer, reader) = tokio::io::duplex(8192);

            let client = reqwest::Client::new();

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
                    return Err(
                        format!("Upload failed with status: {}", response.status()).into(),
                    );
                }

                Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
            });

            stream_zip_to_writer(files, compression, writer).await?;

            upload_task
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?
                .map_err(|e| anyhow!(e))?;
        }
        OutSink::UploadViaFtp {
            host,
            port,
            remote_path,
            user,
            password,
            secure,
        } => {
            let (writer, reader) = tokio::io::duplex(8192);

            let upload_task = tokio::spawn(async move {
                send_net::upload_via_ftp(
                    reader,
                    &host,
                    port,
                    &remote_path,
                    &user,
                    &password,
                    secure,
                )
                .await
            });

            stream_zip_to_writer(files, compression, writer).await?;

            upload_task
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?
                .map_err(|e| anyhow!(e))?;
        }
        OutSink::UploadToS3 {
            bucket,
            key,
            region,
            endpoint,
            access_key,
            secret_key,
        } => {
            let (writer, reader) = tokio::io::duplex(8192);

            let upload_task = tokio::spawn(async move {
                send_s3::upload_to_s3(
                    reader,
                    &bucket,
                    &key,
                    &region,
                    endpoint.as_deref(),
                    access_key.as_deref(),
                    secret_key.as_deref(),
                )
                .await
            });

            stream_zip_to_writer(files, compression, writer).await?;

            upload_task
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?
                .map_err(|e| anyhow!(e))?;
        }
    }

    Ok(())
}
