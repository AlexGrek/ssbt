use std::{
    fs,
    path::{Path, PathBuf},
};
use anyhow::{anyhow, Context, Result};
use crate::Config;

/// Recursively collect all files and empty directories from paths in config.
pub fn list_total_files(config: &Config) -> Result<Vec<PathBuf>> {
    let mut all: Vec<PathBuf> = Vec::new();

    let paths = config
        .paths
        .as_ref()
        .ok_or_else(|| anyhow!("No paths specified"))?;

    for path in paths {
        let path = Path::new(path);
        if !path.exists() {
            eprintln!("Warning: path not found: {}", path.display());
            continue;
        }
        if path.is_file() {
            all.push(path.to_path_buf());
        } else if path.is_dir() {
            walk_dir(path, &mut all)?;
        }
    }

    Ok(all)
}

/// Helper: recursive walk that includes files and empty dirs.
fn walk_dir(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let mut has_entries = false;
    for entry in fs::read_dir(dir).with_context(|| format!("Reading dir {}", dir.display()))? {
        has_entries = true;
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            out.push(path);
        } else if path.is_dir() {
            walk_dir(&path, out)?;
        }
    }

    // include empty dirs
    if !has_entries {
        out.push(dir.to_path_buf());
    }
    Ok(())
}

/// Compute total size of all files and check against max_size limit.
/// If exceeded, exits with code 42.
pub fn total_size(config: &Config, files: &[PathBuf]) -> Result<u64> {
    let mut total: u64 = 0;
    for path in files {
        if path.is_file() {
            let meta = fs::metadata(path)?;
            total += meta.len();
        }
    }

    if let Some(limit_str) = get_max_size_str(config) {
        let limit = parse_size(&limit_str)?;
        if limit > 0 && total > limit {
            eprintln!(
                "Error: total size {} bytes exceeds limit {} ({} bytes)",
                total, limit_str, limit
            );
            std::process::exit(42);
        }
    }

    Ok(total)
}

/// Extract max_size either as numeric or from string units like 10Mi, 5Gi
fn get_max_size_str(config: &Config) -> Option<String> {
    if let Some(val) = config.max_size {
        if val > 0 {
            return Some(val.to_string());
        }
    }
    None
}

/// Parse human-readable sizes in both binary (Ki/Mi/Gi) and decimal (KB/MB/GB) units.
/// Examples: "512Mi", "10Gi", "1MB", "500kb", "1024", "2.5GB"
fn parse_size(s: &str) -> Result<u64> {
    let s = s.trim().to_ascii_lowercase();

    let (multiplier, number_str) = if s.ends_with("ki") {
        (1024_u64, &s[..s.len() - 2])
    } else if s.ends_with("mi") {
        (1024_u64.pow(2), &s[..s.len() - 2])
    } else if s.ends_with("gi") {
        (1024_u64.pow(3), &s[..s.len() - 2])
    } else if s.ends_with("ti") {
        (1024_u64.pow(4), &s[..s.len() - 2])
    } else if s.ends_with("kb") {
        (1000_u64, &s[..s.len() - 2])
    } else if s.ends_with("mb") {
        (1000_u64.pow(2), &s[..s.len() - 2])
    } else if s.ends_with("gb") {
        (1000_u64.pow(3), &s[..s.len() - 2])
    } else if s.ends_with("tb") {
        (1000_u64.pow(4), &s[..s.len() - 2])
    } else {
        (1_u64, s.as_str())
    };

    let number: f64 = number_str
        .trim()
        .parse()
        .with_context(|| format!("Invalid size format: {}", s))?;

    Ok((number * multiplier as f64) as u64)
}

/// Convert bytes into a human-friendly string using binary (KiB, MiB, GiB...) units.
pub fn encode_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    // Format with one decimal if needed (e.g., 1.0 MiB → 1 MiB)
    if (size * 10.0) % 10.0 == 0.0 {
        format!("{:.0} {}", size, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}
