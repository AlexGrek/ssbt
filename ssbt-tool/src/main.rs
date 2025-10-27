use clap::Parser;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, fs};
mod fs_utils;
use fs_utils::{list_total_files, total_size};

use crate::fs_utils::encode_size;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct Config {
    output: Option<String>,
    config: Option<String>,
    format: Option<String>,
    authentication: Option<String>,
    protocol: Option<String>,
    dry: Option<bool>,
    max_size: Option<u64>,
    before: Option<String>,
    after: Option<String>,
    paths: Option<Vec<String>>,
    skip: Option<Vec<String>>,
    compress: Option<bool>,
}

#[derive(Parser, Debug)]
#[command(author, version, about = "SSBT CLI Backup Tool", long_about = None)]
pub struct Cli {
    /// Output path (can be defined via config/env)
    #[arg(short, long)]
    pub output: Option<String>,

    /// Configuration file (YAML or JSON)
    #[arg(short, long)]
    pub config: Option<String>,

    /// Output format [zip|7z|tar]
    #[arg(short, long)]
    pub format: Option<String>,

    /// Authentication token
    #[arg(long)]
    pub authentication: Option<String>,

    /// Protocol [http|https|multipart|scp|tus]
    #[arg(long)]
    pub protocol: Option<String>,

    /// Dry run (just list files and parameters)
    #[arg(short, long, action = clap::ArgAction::SetTrue)]
    pub dry: bool,

    /// Max size limit (0 = unlimited)
    #[arg(short, long, default_value_t = 0)]
    pub max_size: u64,

    /// Command to execute before backup
    #[arg(short, long)]
    pub before: Option<String>,

    /// Command to execute after backup
    #[arg(short, long)]
    pub after: Option<String>,

    /// Patterns to skip (can be specified multiple times)
    #[arg(short = 's', long)]
    pub skip: Vec<String>,

    /// Enable compression
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub compress: bool,

    /// Generate YAML config to stdout
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub generate_yaml_config: bool,

    /// Files or directories to backup
    #[arg()]
    pub paths: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Step 1: Read environment
    let env_config = read_env();

    // Step 2: Read config file (if exists)
    let mut file_config = Config::default();
    if let Some(path) = cli.config.clone().or(env_config.config.clone()) {
        file_config = read_config_file(&path)?;
    }

    // Step 3: Merge configs: env < file < CLI
    let mut merged = merge_configs(env_config, file_config, cli_to_config(&cli));

    // Apply defaults for optional parameters
    if merged.format.is_none() {
        merged.format = Some("zip".to_string());
    }
    if merged.protocol.is_none() {
        merged.protocol = Some("http".to_string());
    }
    if merged.compress.is_none() {
        merged.compress = Some(false);
    }

    // Validate required fields (after merging all sources)
    if merged.output.as_deref().unwrap_or("").is_empty() {
        eprintln!("Error: output path (--output or config:output or SSBT_OUTPUT) is required");
        std::process::exit(2);
    }

    if merged.paths.as_ref().map(|p| p.is_empty()).unwrap_or(true) {
        eprintln!(
            "Error: at least one path must be provided (CLI argument, config:paths, or SSBT_PATHS)"
        );
        std::process::exit(3);
    }

    // Apply defaults only if not defined anywhere
    if merged.format.is_none() {
        merged.format = Some("zip".to_string());
    }
    if merged.protocol.is_none() {
        merged.protocol = Some("http".to_string());
    }

    // Generate YAML config if requested
    if cli.generate_yaml_config {
        let yaml = serde_yaml::to_string(&merged)?;
        println!("{yaml}");
        return Ok(());
    }

    // Dry run: just list parameters
    if merged.dry.unwrap_or(false) {
        println!("--- DRY RUN ---");
        println!("{}", serde_yaml::to_string(&merged)?);
        let files = list_total_files(&merged)?;
        let total = total_size(&merged, &files)?;
        println!("Total files: {}", files.len());
        println!("Total size: {} bytes", encode_size(total));
        for f in files {
            println!("{}", f.display());
        }
        return Ok(());
    }

    println!("No --dry specified, would perform backup here.");
    Ok(())
}

/// Reads environment variables prefixed with SSBT_
fn read_env() -> Config {
    let mut cfg = Config::default();
    let vars: HashMap<String, String> = env::vars().collect();

    macro_rules! get_env {
        ($key:expr) => {
            vars.get(&format!("SSBT_{}", $key)).cloned()
        };
    }

    cfg.output = get_env!("OUTPUT");
    cfg.config = get_env!("CONFIG");
    cfg.format = get_env!("FORMAT");
    cfg.authentication = get_env!("AUTHENTICATION");
    cfg.protocol = get_env!("PROTOCOL");
    cfg.before = get_env!("BEFORE");
    cfg.after = get_env!("AFTER");
    cfg.max_size = get_env!("MAX_SIZE").and_then(|v| v.parse().ok());
    cfg.dry = get_env!("DRY").map(|v| v == "true" || v == "1" || v.eq_ignore_ascii_case("yes"));
    cfg.skip = get_env!("SKIP").map(|v| {
        v.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    });
    cfg.compress =
        get_env!("COMPRESS").map(|v| v == "true" || v == "1" || v.eq_ignore_ascii_case("yes"));
    cfg
}

/// Reads YAML or JSON config from file
fn read_config_file(path: &str) -> anyhow::Result<Config> {
    let content = fs::read_to_string(path)?;
    let lower = path.to_lowercase();
    let cfg = if lower.ends_with(".json") {
        serde_json::from_str(&content)?
    } else {
        serde_yaml::from_str(&content)?
    };
    Ok(cfg)
}

/// Converts CLI struct into Config
fn cli_to_config(cli: &Cli) -> Config {
    Config {
        output: cli.output.clone(),
        config: cli.config.clone(),
        format: cli.format.clone(),
        authentication: cli.authentication.clone(),
        protocol: cli.protocol.clone(),
        dry: Some(cli.dry),
        max_size: Some(cli.max_size),
        before: cli.before.clone(),
        after: cli.after.clone(),
        paths: if cli.paths.is_empty() {
            None
        } else {
            Some(cli.paths.clone())
        },
        skip: if cli.skip.is_empty() {
            None
        } else {
            Some(cli.skip.clone())
        },
        compress: Some(cli.compress),
    }
}

/// Merge configs by priority: env < file < cli
fn merge_configs(env: Config, file: Config, cli: Config) -> Config {
    fn pick<T: Clone>(env: Option<T>, file: Option<T>, cli: Option<T>) -> Option<T> {
        cli.or(file).or(env)
    }

    Config {
        output: pick(env.output, file.output, cli.output),
        config: pick(env.config, file.config, cli.config),
        format: pick(env.format, file.format, cli.format),
        authentication: pick(env.authentication, file.authentication, cli.authentication),
        protocol: pick(env.protocol, file.protocol, cli.protocol),
        dry: pick(env.dry, file.dry, cli.dry),
        max_size: pick(env.max_size, file.max_size, cli.max_size),
        before: pick(env.before, file.before, cli.before),
        after: pick(env.after, file.after, cli.after),
        paths: pick(env.paths, file.paths, cli.paths),
        skip: pick(env.skip, file.skip, cli.skip),
        compress: pick(env.compress, file.compress, cli.compress),
    }
}
