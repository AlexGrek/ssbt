use crate::Config;
use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

use glob::Pattern;

/// Recursively lists all files from `config.paths`, excluding any that match `config.skip` patterns.
pub fn list_total_files(config: &Config) -> Result<Vec<PathBuf>> {
    let mut result = Vec::new();

    // Compile skip patterns with proper error handling
    let skip_patterns: Vec<Pattern> = config
        .skip
        .as_ref()
        .map(|patterns| {
            patterns
                .iter()
                .map(|p| Pattern::new(p).with_context(|| format!("invalid skip pattern: {p}")))
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?
        .unwrap_or_default();

    fn is_skipped(path: &Path, patterns: &[Pattern]) -> bool {
        let path_str = path.to_string_lossy();
        patterns.iter().any(|p| p.matches(&path_str))
    }

    fn walk_dir(dir: &Path, patterns: &[Pattern], result: &mut Vec<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(dir).with_context(|| format!("reading directory {dir:?}"))? {
            let entry = entry?;
            let path = entry.path();

            if is_skipped(&path, patterns) {
                continue;
            }

            if path.is_dir() {
                walk_dir(&path, patterns, result)?;
            } else {
                result.push(path);
            }
        }
        Ok(())
    }

    if let Some(paths) = &config.paths {
        for p in paths {
            let path = PathBuf::from(p);
            if !path.exists() {
                continue;
            }
            if path.is_file() {
                if !is_skipped(&path, &skip_patterns) {
                    result.push(path);
                }
            } else {
                walk_dir(&path, &skip_patterns, &mut result)?;
            }
        }
    }

    Ok(result)
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
