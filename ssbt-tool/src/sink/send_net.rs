use anyhow::Result;
use bytes::Bytes;
use chrono::DateTime;
use futures::{Stream, StreamExt}; // Use futures::Stream
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf}; // Import Path
use tokio::sync::mpsc;
use tokio::task;
use tokio_stream::wrappers::ReceiverStream;

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
