# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

SSBT (Simple Secure Backup Tool) is a Rust CLI backup utility that creates ZIP archives from specified paths with support for glob-based skip patterns, compression, size limits, pre/post hooks, and HTTP upload.

## Build Commands

```bash
cargo build                          # Debug build
cargo build --release                # Release build
cargo test                           # Run all tests
cargo run -- --dry --output test.zip ./  # Dev test with dry run

# Cross-platform release builds (from ssbt-tool/)
cd ssbt-tool && make all             # Build all platforms
cd ssbt-tool && make linux-x86_64    # Single platform
cd ssbt-tool && make clean           # Clean releases
```

## Workspace Structure

Cargo workspace with 3 crates:

- **ssbt-lib** — Shared `Config` struct with serde derives, used by all crates
- **ssbt-tool** — Main CLI binary (the primary crate where most development happens)
- **ssbt-server** — Stub (just prints "Hello world")

## Architecture (ssbt-tool)

Config is resolved from three sources merged by priority: **CLI args > config file (YAML/JSON) > environment variables** (`SSBT_*` prefix). The merge logic is in `main.rs` using a `pick()` helper that takes `cli.or(file).or(env)`.

Key modules in `ssbt-tool/src/`:

| Module | Role |
|---|---|
| `main.rs` | CLI parsing (clap derive), config merging, entry point orchestration |
| `process.rs` | Orchestrates backup: determines output sink (file vs URL), streams files into ZIP |
| `fs_utils.rs` | Recursive file listing with glob skip patterns, size calculation, human-readable size encoding (supports Ki/Mi/Gi and KB/MB/GB units) |
| `naming.rs` | Output filename template expansion (`%datetime%`, `%rand%`, `%pwd%`, `%unix%`, etc.) |
| `shell_exec.rs` | Executes pre/post hook shell commands with real-time stdout streaming |
| `packaging/zip.rs` | Async ZIP creation using `async-zip` with optional DEFLATE compression |
| `packaging/tar.rs` | TAR support — **stub, not implemented** |
| `sink/mod.rs` | `OutSink` enum routing to file or network output |
| `sink/save_file.rs` | File writer with automatic parent directory creation |
| `sink/send_net.rs` | Network upload — **stub, not implemented** |

## Key Design Decisions

- **Async-first**: Uses Tokio runtime for all I/O. ZIP streaming is fully async via `async-zip`.
- **Memory-efficient streaming**: Archives are streamed through a tokio duplex pipe rather than buffered in memory, both for file output and HTTP upload.
- **Exit code 42**: Used when backup exceeds `max_size` limit.
- **Compression off by default**: Must be explicitly enabled with `--compress`.
- **Rust 2024 edition**.
