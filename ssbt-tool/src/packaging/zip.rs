use std::fs::File;
use std::io::{self, Read, Write, BufReader};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const BUFFER_SIZE: usize = 64 * 1024; // 64KB buffer for streaming

/// Compression method for ZIP archive
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    /// No compression (store only) - streaming compatible
    Store,
    /// DEFLATE compression - streaming compatible
    Deflate,
}

/// Configuration for adding a file to the archive
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Source file path on filesystem
    pub source_path: PathBuf,
    /// Path to use inside the ZIP archive
    pub archive_path: String,
}

impl FileEntry {
    /// Create a new file entry
    pub fn new(source_path: impl Into<PathBuf>, archive_path: impl Into<String>) -> Self {
        Self {
            source_path: source_path.into(),
            archive_path: archive_path.into(),
        }
    }

    /// Create a file entry using just the filename for the archive path
    pub fn with_filename(source_path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = source_path.into();
        let archive_path = path
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid file path"))?
            .to_string_lossy()
            .to_string();
        Ok(Self {
            source_path: path,
            archive_path,
        })
    }

    /// Create a file entry with path relative to a base directory
    pub fn with_base_dir(source_path: impl Into<PathBuf>, base_dir: &Path) -> io::Result<Self> {
        let path = source_path.into();
        let archive_path = path
            .strip_prefix(base_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        Ok(Self {
            source_path: path,
            archive_path,
        })
    }
}

struct CentralDirectoryEntry {
    file_name: String,
    compressed_size: u64,
    uncompressed_size: u64,
    crc32: u32,
    local_header_offset: u64,
    compression_method: u16,
    modified_time: u16,
    modified_date: u16,
}

/// DOS date/time encoding
fn dos_datetime(system_time: SystemTime) -> (u16, u16) {
    let duration = system_time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs();
    
    // Convert to DOS format (very simplified)
    let time = 0u16; // 00:00:00
    let date = 0x21u16; // 1980-01-01
    
    (time, date)
}

fn write_u16<W: Write>(writer: &mut W, value: u16) -> io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}

fn write_u32<W: Write>(writer: &mut W, value: u32) -> io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}

fn write_u64<W: Write>(writer: &mut W, value: u64) -> io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}

/// Calculate CRC32 checksum
fn crc32(data: &[u8], current: u32) -> u32 {
    let mut crc = current;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

/// Write local file header for streaming (using data descriptor)
fn write_local_header<W: Write>(
    writer: &mut W,
    file_name: &str,
    compression_method: u16,
    modified_time: u16,
    modified_date: u16,
) -> io::Result<()> {
    // Local file header signature
    write_u32(writer, 0x04034b50)?;
    
    // Version needed to extract (2.0)
    write_u16(writer, 20)?;
    
    // General purpose bit flag (bit 3 = data descriptor used)
    write_u16(writer, 0x0008)?;
    
    // Compression method
    write_u16(writer, compression_method)?;
    
    // Last mod file time & date
    write_u16(writer, modified_time)?;
    write_u16(writer, modified_date)?;
    
    // CRC-32 (set to 0, will be in data descriptor)
    write_u32(writer, 0)?;
    
    // Compressed size (set to 0, will be in data descriptor)
    write_u32(writer, 0)?;
    
    // Uncompressed size (set to 0, will be in data descriptor)
    write_u32(writer, 0)?;
    
    // File name length
    write_u16(writer, file_name.len() as u16)?;
    
    // Extra field length
    write_u16(writer, 0)?;
    
    // File name
    writer.write_all(file_name.as_bytes())?;
    
    Ok(())
}

/// Write data descriptor after file data
fn write_data_descriptor<W: Write>(
    writer: &mut W,
    crc32: u32,
    compressed_size: u64,
    uncompressed_size: u64,
) -> io::Result<()> {
    // Data descriptor signature (optional but recommended)
    write_u32(writer, 0x08074b50)?;
    
    // CRC-32
    write_u32(writer, crc32)?;
    
    // Compressed size (use ZIP64 format for large files)
    write_u64(writer, compressed_size)?;
    
    // Uncompressed size
    write_u64(writer, uncompressed_size)?;
    
    Ok(())
}

/// Write central directory file header
fn write_central_directory_header<W: Write>(
    writer: &mut W,
    entry: &CentralDirectoryEntry,
) -> io::Result<()> {
    // Central directory file header signature
    write_u32(writer, 0x02014b50)?;
    
    // Version made by (Unix, version 2.0)
    write_u16(writer, 0x031E)?;
    
    // Version needed to extract (2.0)
    write_u16(writer, 20)?;
    
    // General purpose bit flag (data descriptor used)
    write_u16(writer, 0x0008)?;
    
    // Compression method
    write_u16(writer, entry.compression_method)?;
    
    // Last mod file time & date
    write_u16(writer, entry.modified_time)?;
    write_u16(writer, entry.modified_date)?;
    
    // CRC-32
    write_u32(writer, entry.crc32)?;
    
    // Compressed size
    write_u32(writer, entry.compressed_size.min(0xFFFFFFFF) as u32)?;
    
    // Uncompressed size
    write_u32(writer, entry.uncompressed_size.min(0xFFFFFFFF) as u32)?;
    
    // File name length
    write_u16(writer, entry.file_name.len() as u16)?;
    
    // Extra field length (20 bytes for ZIP64)
    let needs_zip64 = entry.compressed_size > 0xFFFFFFFF || 
                      entry.uncompressed_size > 0xFFFFFFFF ||
                      entry.local_header_offset > 0xFFFFFFFF;
    write_u16(writer, if needs_zip64 { 20 } else { 0 })?;
    
    // File comment length
    write_u16(writer, 0)?;
    
    // Disk number start
    write_u16(writer, 0)?;
    
    // Internal file attributes
    write_u16(writer, 0)?;
    
    // External file attributes (Unix permissions)
    write_u32(writer, 0o644 << 16)?;
    
    // Relative offset of local header
    write_u32(writer, entry.local_header_offset.min(0xFFFFFFFF) as u32)?;
    
    // File name
    writer.write_all(entry.file_name.as_bytes())?;
    
    // Extra field (ZIP64)
    if needs_zip64 {
        write_u16(writer, 0x0001)?; // ZIP64 extended information
        write_u16(writer, 16)?; // Size of this extra block
        write_u64(writer, entry.uncompressed_size)?;
        write_u64(writer, entry.compressed_size)?;
    }
    
    Ok(())
}

/// Write end of central directory record
fn write_end_of_central_directory<W: Write>(
    writer: &mut W,
    num_entries: u16,
    central_dir_size: u64,
    central_dir_offset: u64,
) -> io::Result<()> {
    // End of central directory signature
    write_u32(writer, 0x06054b50)?;
    
    // Number of this disk
    write_u16(writer, 0)?;
    
    // Disk where central directory starts
    write_u16(writer, 0)?;
    
    // Number of central directory records on this disk
    write_u16(writer, num_entries)?;
    
    // Total number of central directory records
    write_u16(writer, num_entries)?;
    
    // Size of central directory
    write_u32(writer, central_dir_size.min(0xFFFFFFFF) as u32)?;
    
    // Offset of start of central directory
    write_u32(writer, central_dir_offset.min(0xFFFFFFFF) as u32)?;
    
    // ZIP file comment length
    write_u16(writer, 0)?;
    
    Ok(())
}

/// Stream files into a ZIP archive with minimal memory usage
///
/// This function creates a ZIP archive by streaming files directly from disk to the output
/// writer without loading entire files into memory and without seeking. This allows 
/// processing files larger than available RAM and streaming to network sockets, pipes, etc.
///
/// # Arguments
/// * `writer` - Any writer implementing Write trait (file, stdout, network socket, etc.)
/// * `compression` - Compression method to use (Store or Deflate)
/// * `files` - List of file entries to include in the archive
///
/// # Returns
/// Result indicating success or an IO error
///
/// # Examples
///
/// ```no_run
/// use std::fs::File;
/// use streaming_zip::{package_zip_streaming, Compression, FileEntry};
///
/// // Write to a file
/// let output = File::create("backup.zip")?;
/// let files = vec![
///     FileEntry::new("/data/large_file.bin", "large_file.bin"),
///     FileEntry::new("/logs/app.log", "logs/app.log"),
/// ];
/// package_zip_streaming(output, Compression::Store, &files)?;
///
/// // Stream to stdout
/// let stdout = std::io::stdout();
/// package_zip_streaming(stdout.lock(), Compression::Store, &files)?;
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn package_zip_streaming<W: Write>(
    mut writer: W,
    compression: Compression,
    files: &[FileEntry],
) -> io::Result<()> {
    let mut central_directory_entries = Vec::new();
    let mut current_offset = 0u64;
    
    // Process each file
    for entry in files {
        if !entry.source_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("File not found: {:?}", entry.source_path)
            ));
        }

        if !entry.source_path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Not a regular file: {:?}", entry.source_path)
            ));
        }

        let metadata = entry.source_path.metadata()?;
        let modified = metadata.modified().unwrap_or(SystemTime::now());
        let (mod_time, mod_date) = dos_datetime(modified);
        
        let compression_method = match compression {
            Compression::Store => 0,
            Compression::Deflate => 8,
        };
        
        let local_header_offset = current_offset;
        
        // Write local file header
        write_local_header(&mut writer, &entry.archive_path, compression_method, mod_time, mod_date)?;
        current_offset += 30 + entry.archive_path.len() as u64;
        
        // Stream file data
        let file = File::open(&entry.source_path)?;
        let mut reader = BufReader::with_capacity(BUFFER_SIZE, file);
        let mut buffer = vec![0u8; BUFFER_SIZE];
        let mut crc = 0xFFFFFFFF;
        let mut uncompressed_size = 0u64;
        let mut compressed_size = 0u64;
        
        match compression {
            Compression::Store => {
                // No compression - just copy
                loop {
                    let bytes_read = reader.read(&mut buffer)?;
                    if bytes_read == 0 {
                        break;
                    }
                    crc = crc32(&buffer[..bytes_read], crc);
                    writer.write_all(&buffer[..bytes_read])?;
                    uncompressed_size += bytes_read as u64;
                    compressed_size += bytes_read as u64;
                }
            }
            Compression::Deflate => {
                // Use flate2 for streaming deflate compression
                use flate2::write::DeflateEncoder;
                use flate2::Compression as FlateCompression;
                
                let mut encoder = DeflateEncoder::new(&mut writer, FlateCompression::default());
                
                loop {
                    let bytes_read = reader.read(&mut buffer)?;
                    if bytes_read == 0 {
                        break;
                    }
                    crc = crc32(&buffer[..bytes_read], crc);
                    encoder.write_all(&buffer[..bytes_read])?;
                    uncompressed_size += bytes_read as u64;
                }
                
                compressed_size = encoder.finish()?.len() as u64 - current_offset - 30 - entry.archive_path.len() as u64;
            }
        }
        
        current_offset += compressed_size;
        
        crc = !crc;
        
        // Write data descriptor
        write_data_descriptor(&mut writer, crc, compressed_size, uncompressed_size)?;
        current_offset += 20; // Size of data descriptor with ZIP64
        
        // Store for central directory
        central_directory_entries.push(CentralDirectoryEntry {
            file_name: entry.archive_path.clone(),
            compressed_size,
            uncompressed_size,
            crc32: crc,
            local_header_offset,
            compression_method,
            modified_time: mod_time,
            modified_date: mod_date,
        });
    }
    
    let central_dir_offset = current_offset;
    
    // Write central directory
    for entry in &central_directory_entries {
        write_central_directory_header(&mut writer, entry)?;
        current_offset += 46 + entry.file_name.len() as u64;
        if entry.compressed_size > 0xFFFFFFFF || 
           entry.uncompressed_size > 0xFFFFFFFF ||
           entry.local_header_offset > 0xFFFFFFFF {
            current_offset += 20; // ZIP64 extra field
        }
    }
    
    let central_dir_size = current_offset - central_dir_offset;
    
    // Write end of central directory
    write_end_of_central_directory(
        &mut writer,
        central_directory_entries.len() as u16,
        central_dir_size,
        central_dir_offset,
    )?;
    
    Ok(())
}

/// Stream files into a ZIP archive from a simple list of paths
///
/// Convenience function that uses just the filename for each file's archive path.
///
/// # Arguments
/// * `writer` - Any writer implementing Write trait
/// * `compression` - Compression method to use
/// * `file_paths` - List of file paths to include
///
/// # Examples
///
/// ```no_run
/// use std::fs::File;
/// use std::path::PathBuf;
/// use streaming_zip::{package_zip_simple, Compression};
///
/// let output = File::create("backup.zip")?;
/// let files = vec![
///     PathBuf::from("/data/file1.txt"),
///     PathBuf::from("/data/file2.txt"),
/// ];
/// package_zip_simple(output, Compression::Store, &files)?;
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn package_zip_simple<W: Write>(
    writer: W,
    compression: Compression,
    file_paths: &[PathBuf],
) -> io::Result<()> {
    let entries: Result<Vec<_>, _> = file_paths
        .iter()
        .map(|path| FileEntry::with_filename(path))
        .collect();
    
    package_zip_streaming(writer, compression, &entries?)
}

/// Stream files into a ZIP archive with paths relative to a base directory
///
/// # Arguments
/// * `writer` - Any writer implementing Write trait
/// * `compression` - Compression method to use
/// * `file_paths` - List of file paths to include
/// * `base_dir` - Base directory for calculating relative paths
///
/// # Examples
///
/// ```no_run
/// use std::fs::File;
/// use std::path::{Path, PathBuf};
/// use streaming_zip::{package_zip_with_base, Compression};
///
/// let output = File::create("backup.zip")?;
/// let files = vec![
///     PathBuf::from("/data/logs/app.log"),
///     PathBuf::from("/data/logs/error.log"),
/// ];
/// package_zip_with_base(output, Compression::Store, &files, Path::new("/data"))?;
/// // Results in: logs/app.log, logs/error.log in the archive
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn package_zip_with_base<W: Write>(
    writer: W,
    compression: Compression,
    file_paths: &[PathBuf],
    base_dir: &Path,
) -> io::Result<()> {
    let entries: Result<Vec<_>, _> = file_paths
        .iter()
        .map(|path| FileEntry::with_base_dir(path, base_dir))
        .collect();
    
    package_zip_streaming(writer, compression, &entries?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc32() {
        let data = b"Hello, World!";
        let crc = !crc32(data, 0xFFFFFFFF);
        assert_ne!(crc, 0);
    }

    #[test]
    fn test_empty_archive() {
        let mut buffer = Vec::new();
        let files = vec![];
        let result = package_zip_streaming(&mut buffer, Compression::Store, &files);
        assert!(result.is_ok());
        assert!(buffer.len() > 0);
    }

    #[test]
    fn test_file_entry_new() {
        let entry = FileEntry::new("/path/to/file.txt", "file.txt");
        assert_eq!(entry.source_path, PathBuf::from("/path/to/file.txt"));
        assert_eq!(entry.archive_path, "file.txt");
    }
}
