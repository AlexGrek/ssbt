use std::path::{Path, PathBuf};

use async_zip::Compression;

use crate::{
    Config,
    sink::{OutSink, stream_zip_to_sink},
};

fn get_output_sink(config: &Config) -> Result<OutSink, Box<dyn std::error::Error>> {
    match &config.output {
        Some(output) => {
            if output.starts_with("http://") || output.starts_with("https://") {
                Ok(OutSink::UploadToUrl(output.clone()))
            } else {
                Ok(OutSink::SaveToFile(PathBuf::from(output)))
            }
        }
        None => {
            // Default: save to "archive.zip" in current directory
            Ok(OutSink::SaveToFile(PathBuf::from("archive.zip")))
        }
    }
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

    let compression_decision = config.compress.unwrap_or(false);

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
