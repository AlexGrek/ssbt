use crate::naming::create_file_name;
use std::path::{Path, PathBuf};

use async_zip::Compression;

use crate::{
    Config,
    sink::{OutSink, stream_zip_to_sink},
};

fn get_output_sink(config: &Config) -> Result<OutSink, Box<dyn std::error::Error>> {
    let protocol = config.protocol.as_deref().unwrap_or("http");
    let output = config.output.as_deref().unwrap_or(".");

    match protocol {
        "ftp" | "ftps" => {
            // output format: host:port/remote/path/file.zip  or  host/remote/path/file.zip
            let (host, port, remote_path) = parse_ftp_output(output)?;
            Ok(OutSink::UploadViaFtp {
                host,
                port,
                remote_path,
                user: config.ftp_user.clone().unwrap_or_else(|| "anonymous".into()),
                password: config.ftp_password.clone().unwrap_or_default(),
                secure: protocol == "ftps",
            })
        }
        "s3" => {
            // output format: bucket/key/path.zip
            let (bucket, key) = parse_s3_output(output)?;
            Ok(OutSink::UploadToS3 {
                bucket,
                key,
                region: config
                    .s3_region
                    .clone()
                    .unwrap_or_else(|| "us-east-1".into()),
                endpoint: config.s3_endpoint.clone(),
                access_key: config.s3_access_key.clone(),
                secret_key: config.s3_secret_key.clone(),
            })
        }
        "scp" | "ssh" | "sftp" => {
            Err(format!(
                "Protocol '{}' is not supported: no pure-Rust async streaming library available",
                protocol
            )
            .into())
        }
        "http" | "https" => {
            if output.starts_with("http://") || output.starts_with("https://") {
                Ok(OutSink::UploadToUrl(output.to_string()))
            } else {
                Ok(OutSink::SaveToFile(create_file_name(output)?))
            }
        }
        _ => Ok(OutSink::SaveToFile(create_file_name(output)?)),
    }
}

/// Parses FTP output string into (host, port, remote_path).
/// Accepted formats: `host:port/path`, `host/path`.
fn parse_ftp_output(output: &str) -> Result<(String, u16, String), Box<dyn std::error::Error>> {
    let output = output
        .trim_start_matches("ftp://")
        .trim_start_matches("ftps://");

    let (host_port, remote_path) = output
        .split_once('/')
        .ok_or("FTP output must be in format host[:port]/remote/path/file.zip")?;

    let (host, port) = if let Some((h, p)) = host_port.split_once(':') {
        (h.to_string(), p.parse::<u16>()?)
    } else {
        (host_port.to_string(), 21)
    };

    Ok((host, port, format!("/{}", remote_path)))
}

/// Parses S3 output string into (bucket, key).
/// Accepted formats: `bucket/key/path.zip`, `s3://bucket/key/path.zip`.
fn parse_s3_output(output: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    let output = output.trim_start_matches("s3://");

    let (bucket, key) = output
        .split_once('/')
        .ok_or("S3 output must be in format bucket/key/path.zip")?;

    if bucket.is_empty() || key.is_empty() {
        return Err("S3 bucket and key must not be empty".into());
    }

    Ok((bucket.to_string(), key.to_string()))
}

fn prepare_entries(files: Vec<PathBuf>, base_path: Option<&Path>) -> Vec<(String, PathBuf)> {
    files
        .into_iter()
        .map(|file_path| {
            // Determine the path to use inside the zip archive
            let archive_name = if let Some(base) = base_path {
                file_path
                    .strip_prefix(base)
                    .unwrap_or(&file_path)
                    .to_string_lossy()
                    .to_string()
            } else {
                // Use just the filename if no base path
                file_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| file_path.to_string_lossy().to_string())
            };

            (archive_name, file_path)
        })
        .collect()
}

pub fn process_files_within_tokio(
    config: Config,
    files: Vec<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all() // Enables both IO and time drivers
        .build()?;
    // Run async function in runtime
    runtime.block_on(async { process_files(config, files).await })
}

async fn process_files(
    config: Config,
    files: Vec<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Determine output sink
    let sink = get_output_sink(&config)?;

    // Get base path for relative archive paths (use first common directory)
    let base_path = find_common_base(&files);

    // Prepare entries for zip
    let entries = prepare_entries(files, base_path.as_deref());

    // Check if dry run
    if config.dry == Some(true) {
        println!(
            "Dry run - would create archive with {} files",
            entries.len()
        );
        for (archive_name, file_path) in &entries {
            println!("  {} -> {}", file_path.display(), archive_name);
        }
        println!("Output: {:?}", sink);
        return Ok(());
    }

    println!("Backup output: {:?}", sink);

    let compression_decision = config.compress.unwrap_or(false);

    if compression_decision {
        println!("Using DEFLATE compression")
    } else {
        println!("Compression disabled")
    }

    let compression = if compression_decision {
        Compression::Deflate
    } else {
        Compression::Stored
    };

    stream_zip_to_sink(entries, compression, sink).await?;
    println!("Archive created successfully!");

    Ok(())
}

fn find_common_base(files: &[PathBuf]) -> Option<PathBuf> {
    if files.is_empty() {
        return None;
    }

    // Start with the parent of the first file
    let mut base = files[0].parent()?.to_path_buf();

    // Find common ancestor
    for file in files.iter().skip(1) {
        while !file.starts_with(&base) {
            base = base.parent()?.to_path_buf();
        }
    }

    Some(base)
}
