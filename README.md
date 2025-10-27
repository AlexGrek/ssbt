# 🚀 SSBT - Simple Secure Backup Tool

A powerful, flexible CLI backup tool built with Rust that supports multiple configuration sources, compression formats, and upload protocols.

## ✨ Features

- 🔧 **Multiple Configuration Sources**: Command-line arguments, config files (YAML/JSON), and environment variables
- 📦 **Multiple Archive Formats**: ZIP, 7z, and TAR support
- 🌐 **Protocol Flexibility**: HTTP, HTTPS, multipart uploads, SCP, and TUS resumable uploads
- 🎯 **Smart Filtering**: Skip patterns to exclude unwanted files
- 💾 **Size Controls**: Set maximum backup size limits
- 🔐 **Secure Authentication**: Token-based authentication support
- 🧪 **Dry Run Mode**: Preview what will be backed up without actually performing the backup
- ⚙️ **Pre/Post Hooks**: Execute commands before and after backup operations

## 📥 Installation

```bash
cargo install ssbt
```

Or build from source:

```bash
git clone https://github.com/yourusername/ssbt
cd ssbt
cargo build --release
```

## 🚀 Quick Start

### Basic Usage

Backup a single directory:

```bash
ssbt --output /backups/mybackup.zip /path/to/directory
```

Backup multiple paths:

```bash
ssbt --output /backups/mybackup.zip /path/to/dir1 /path/to/dir2 /path/to/file.txt
```

### Dry Run

Preview what will be backed up:

```bash
ssbt --dry --output /backups/test.zip /path/to/directory
```

Output:
```
--- DRY RUN ---
output: /backups/test.zip
format: zip
protocol: http
compress: false
paths:
  - /path/to/directory
Total files: 42
Total size: 15.3 MB
/path/to/directory/file1.txt
/path/to/directory/file2.txt
...
```

## 🎛️ Configuration

SSBT supports three configuration sources with the following priority (highest to lowest):

1. **Command-line arguments** (highest priority)
2. **Configuration file** (YAML or JSON)
3. **Environment variables** (lowest priority)

### Command-Line Arguments

```bash
ssbt [OPTIONS] <PATHS>...

Options:
  -o, --output <OUTPUT>              Output path
  -c, --config <CONFIG>              Configuration file (YAML or JSON)
  -f, --format <FORMAT>              Output format [zip|7z|tar] (default: zip)
      --authentication <TOKEN>       Authentication token
      --protocol <PROTOCOL>          Protocol [http|https|multipart|scp|tus] (default: http)
  -d, --dry                          Dry run (just list files and parameters)
  -m, --max-size <SIZE>              Max size limit in bytes (0 = unlimited)
  -b, --before <COMMAND>             Command to execute before backup
  -a, --after <COMMAND>              Command to execute after backup
  -s, --skip <PATTERN>               Patterns to skip (can be specified multiple times)
      --compress                     Enable compression
      --generate-yaml-config         Generate YAML config to stdout
  -h, --help                         Print help
  -V, --version                      Print version

Arguments:
  <PATHS>...                         Files or directories to backup
```

### Configuration File

Create a `backup.yaml` file:

```yaml
output: /backups/mybackup.zip
format: zip
protocol: https
compress: true
authentication: your-secret-token
max_size: 10737418240  # 10 GB in bytes
before: echo "Starting backup..."
after: echo "Backup complete!"
skip:
  - "*.log"
  - "*.tmp"
  - "node_modules"
  - ".git"
  - "target"
paths:
  - /home/user/documents
  - /home/user/projects
  - /etc/nginx
```

Use it:

```bash
ssbt --config backup.yaml
```

Or JSON format (`backup.json`):

```json
{
  "output": "/backups/mybackup.zip",
  "format": "7z",
  "protocol": "https",
  "compress": true,
  "skip": ["*.log", "*.tmp", "node_modules"],
  "paths": ["/home/user/documents", "/home/user/projects"]
}
```

### Environment Variables

All configuration options can be set via environment variables with the `SSBT_` prefix:

```bash
export SSBT_OUTPUT=/backups/mybackup.zip
export SSBT_FORMAT=zip
export SSBT_PROTOCOL=https
export SSBT_AUTHENTICATION=your-secret-token
export SSBT_COMPRESS=true
export SSBT_DRY=false
export SSBT_MAX_SIZE=10737418240
export SSBT_BEFORE="echo 'Starting backup...'"
export SSBT_AFTER="echo 'Backup complete!'"
export SSBT_SKIP="*.log,*.tmp,node_modules,.git"
export SSBT_PATHS="/home/user/documents,/home/user/projects"

ssbt  # Will use environment variables
```

### Generate Configuration

Generate a YAML configuration file from current settings:

```bash
ssbt --output /backups/test.zip --format 7z --compress /path/to/dir --generate-yaml-config > backup.yaml
```

## 🎯 Advanced Usage

### Skip Patterns

Exclude files and directories using multiple skip patterns:

```bash
ssbt --output backup.zip \
  --skip "*.log" \
  --skip "*.tmp" \
  --skip "node_modules" \
  --skip ".git" \
  /path/to/project
```

Or in config file:

```yaml
skip:
  - "*.log"
  - "*.tmp"
  - "*.cache"
  - "node_modules"
  - ".git"
  - "__pycache__"
  - "target"
```

### Compression

Enable compression for reduced backup size:

```bash
ssbt --output backup.zip --compress /path/to/directory
```

### Size Limits

Set a maximum backup size (in bytes):

```bash
ssbt --output backup.zip --max-size 5368709120 /path/to/directory  # 5 GB limit
```

### Pre/Post Backup Hooks

Execute commands before and after backup:

```bash
ssbt --output backup.zip \
  --before "pg_dump mydb > /tmp/db.sql" \
  --after "rm /tmp/db.sql" \
  /tmp/db.sql
```

### Multiple Protocols

Choose your upload protocol:

```bash
# Standard HTTP
ssbt --output https://backup.example.com/upload --protocol http /path/to/dir

# Secure HTTPS
ssbt --output https://backup.example.com/upload --protocol https /path/to/dir

# SCP to remote server
ssbt --output user@server:/backups/backup.zip --protocol scp /path/to/dir

# TUS resumable uploads
ssbt --output https://tus.example.com/files/ --protocol tus /path/to/dir
```

### Authentication

Secure your backups with authentication:

```bash
ssbt --output https://backup.example.com/upload \
  --authentication "Bearer your-secret-token" \
  /path/to/directory
```

## 📝 Examples

### Daily Database Backup

```bash
#!/bin/bash
ssbt --config /etc/ssbt/db-backup.yaml \
  --before "pg_dump -U postgres myapp > /tmp/myapp.sql" \
  --after "rm /tmp/myapp.sql" \
  /tmp/myapp.sql
```

### Automated Project Backup

```yaml
# project-backup.yaml
output: /backups/project-$(date +%Y%m%d).zip
format: zip
compress: true
skip:
  - "node_modules"
  - ".git"
  - "*.log"
  - "dist"
  - "build"
  - ".env"
paths:
  - /home/user/projects/myapp
```

```bash
ssbt --config project-backup.yaml
```

### Multi-Environment Configuration

```bash
# Development
SSBT_OUTPUT=/dev/backups/dev.zip SSBT_PROTOCOL=http ssbt /app

# Production
SSBT_OUTPUT=s3://backups/prod.zip SSBT_PROTOCOL=https SSBT_AUTHENTICATION=$SECRET ssbt /app
```

## 🛠️ Development

### Requirements

- Rust 1.70 or higher
- Cargo

### Build

```bash
cargo build
```

### Run Tests

```bash
cargo test
```

### Run with Development Mode

```bash
cargo run -- --dry --output test.zip ./
```

## 📄 License

MIT License - see LICENSE file for details

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 🐛 Issues

Found a bug? Please [open an issue](https://github.com/yourusername/ssbt/issues) on GitHub.

## 📊 Status

✅ Configuration management
✅ Dry run mode
✅ Multiple input sources
✅ File listing and size calculation
✅ Skip patterns
✅ Compression options
🚧 Archive creation (in progress)
🚧 Upload protocols (in progress)
🚧 Authentication (in progress)

---

Made with ❤️ and Rust
