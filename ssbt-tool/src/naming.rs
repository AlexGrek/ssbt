use anyhow::{Context, Result};
use chrono::{Datelike, Local, Timelike, Utc};
use rand::Rng;
use std::env;
use std::path::{Path, PathBuf};

pub fn create_file_name(input: &str) -> Result<PathBuf> {
    let input_path = Path::new(input);

    // Determine if input ends with a file or a directory
    let is_file = input_path
        .extension()
        .map(|ext| !ext.is_empty())
        .unwrap_or(false);

    let dir = if is_file {
        input_path.parent().unwrap_or_else(|| Path::new("."))
    } else {
        input_path
    };

    // Extract file name template
    let file_name_template = if is_file {
        input_path
            .file_name()
            .context("Invalid file name in path")?
            .to_string_lossy()
            .to_string()
    } else {
        "backup_%datetime%_%rand%.zip".to_string()
    };

    // Current time info
    let now_utc = Utc::now();
    let now_local = Local::now();

    let date = now_utc.format("%Y-%m-%d").to_string();
    let time = now_utc.format("%H-%M-%S").to_string();
    let datetime = now_utc.format("%Y-%m-%d_%H-%M-%S").to_string();
    let weekday = now_utc.format("%a").to_string();

    let rand5 = random_string(5);
    let rand12 = random_string(12);

    let pwd = env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "unknown".into());

    // Replace placeholders (case-insensitive)
    let mut name = file_name_template.clone();
    let replacements = vec![
        ("%datetime%", datetime),
        ("%rand%", rand5),
        ("%longrand%", rand12),
        ("%pwd%", pwd),
        ("%date%", date),
        ("%time%", time),
        ("%hh%", format!("{:02}", now_utc.hour())),
        ("%mm%", format!("{:02}", now_utc.minute())),
        ("%ss%", format!("{:02}", now_utc.second())),
        ("%dd%", format!("{:02}", now_utc.day())),
        ("%ww%", weekday),
        ("%yyyy%", format!("{:04}", now_utc.year())),
        ("%yy%", format!("{:02}", now_utc.year() % 100)),
        ("%ms%", format!("{:03}", now_utc.timestamp_subsec_millis())),
        ("%unix%", format!("{}", now_utc.timestamp())),
        ("%ltime%", now_local.format("%Y-%m-%d_%H-%M-%S").to_string()),
        ("%lh%", format!("{:02}", now_local.hour())),
        ("%ld%", format!("{:02}", now_local.day())),
    ];

    for (pattern, value) in replacements {
        name = replace_case_insensitive(&name, pattern, &value);
    }

    Ok(dir.join(name))
}

/// Generates a random lowercase alphanumeric string.
fn random_string(len: usize) -> String {
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..len)
        .map(|_| {
            let idx = rng.random_range(0..CHARS.len());
            CHARS[idx] as char
        })
        .collect()
}

/// Helper for case-insensitive substring replacement
fn replace_case_insensitive(s: &str, pattern: &str, replacement: &str) -> String {
    let mut result = String::new();
    let lower_s = s.to_lowercase();
    let lower_pattern = pattern.to_lowercase();

    let mut last_end = 0;
    let mut search_start = 0;

    while let Some(pos) = lower_s[search_start..].find(&lower_pattern) {
        let abs_pos = search_start + pos;
        result.push_str(&s[last_end..abs_pos]);
        result.push_str(replacement);
        last_end = abs_pos + pattern.len();
        search_start = last_end;
    }

    result.push_str(&s[last_end..]);
    result
}
