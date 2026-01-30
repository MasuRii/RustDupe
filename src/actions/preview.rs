//! File preview functionality.
//!
//! This module provides file preview capabilities for the TUI:
//! - Text file content preview (first 50 lines)
//! - Binary file hex dump (first 256 bytes)
//! - Image file metadata (dimensions, format, size)
//!
//! # Performance
//!
//! All preview functions limit data to the first 4KB for fast loading.
//! This ensures responsive UI even for very large files.
//!
//! # Example
//!
//! ```
//! use rustdupe::actions::preview::{preview_file, PreviewContent, PreviewType};
//! use std::path::Path;
//!
//! let result = preview_file(Path::new("test.txt"));
//! match result {
//!     Ok(content) => match content.preview_type {
//!         PreviewType::Text => println!("Text: {}", content.content),
//!         PreviewType::Binary => println!("Binary: {}", content.content),
//!         PreviewType::Image => println!("Image: {}", content.content),
//!         PreviewType::Empty => println!("Empty file"),
//!         PreviewType::Error => println!("Error: {}", content.content),
//!     },
//!     Err(e) => println!("Preview failed: {}", e),
//! }
//! ```

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use thiserror::Error;

/// Maximum bytes to read for preview (4KB).
const MAX_PREVIEW_BYTES: usize = 4096;

/// Maximum lines to show for text preview.
const MAX_PREVIEW_LINES: usize = 50;

/// Bytes to read for hex dump preview.
const HEX_DUMP_BYTES: usize = 256;

/// Bytes to sample for binary detection.
const BINARY_DETECT_BYTES: usize = 512;

/// Known text file extensions.
const TEXT_EXTENSIONS: &[&str] = &[
    "txt",
    "md",
    "rs",
    "py",
    "js",
    "ts",
    "tsx",
    "jsx",
    "json",
    "xml",
    "html",
    "css",
    "yml",
    "yaml",
    "toml",
    "cfg",
    "ini",
    "conf",
    "sh",
    "bash",
    "zsh",
    "fish",
    "ps1",
    "bat",
    "cmd",
    "c",
    "cpp",
    "h",
    "hpp",
    "java",
    "kt",
    "go",
    "rb",
    "php",
    "pl",
    "sql",
    "r",
    "m",
    "swift",
    "lua",
    "vim",
    "log",
    "csv",
    "tsv",
    "env",
    "gitignore",
    "dockerignore",
    "makefile",
    "cmake",
];

/// Known image file extensions.
const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "webp", "svg", "tiff", "tif", "heic", "heif", "raw",
    "cr2", "nef", "arw", "dng",
];

/// Errors that can occur during file preview.
#[derive(Debug, Error)]
pub enum PreviewError {
    /// File was not found.
    #[error("file not found: {0}")]
    NotFound(String),

    /// Permission was denied when reading the file.
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

/// Type of preview content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewType {
    /// Text file content.
    Text,
    /// Binary file hex dump.
    Binary,
    /// Image file metadata.
    Image,
    /// Empty file.
    Empty,
    /// Error message.
    Error,
}

/// Preview content with type information.
#[derive(Debug, Clone)]
pub struct PreviewContent {
    /// The preview type.
    pub preview_type: PreviewType,
    /// The preview content string.
    pub content: String,
    /// File size in bytes.
    pub file_size: u64,
    /// Optional file metadata.
    pub metadata: Option<PreviewMetadata>,
}

/// Additional metadata for preview.
#[derive(Debug, Clone)]
pub struct PreviewMetadata {
    /// File extension (lowercase).
    pub extension: Option<String>,
    /// Number of lines (for text files).
    pub line_count: Option<usize>,
    /// Image dimensions (width, height) if available.
    pub dimensions: Option<(u32, u32)>,
}

impl PreviewContent {
    /// Create a new text preview.
    #[must_use]
    pub fn text(content: String, file_size: u64, line_count: usize) -> Self {
        Self {
            preview_type: PreviewType::Text,
            content,
            file_size,
            metadata: Some(PreviewMetadata {
                extension: None,
                line_count: Some(line_count),
                dimensions: None,
            }),
        }
    }

    /// Create a new binary preview with hex dump.
    #[must_use]
    pub fn binary(hex_dump: String, file_size: u64) -> Self {
        Self {
            preview_type: PreviewType::Binary,
            content: hex_dump,
            file_size,
            metadata: None,
        }
    }

    /// Create an image preview with metadata.
    #[must_use]
    pub fn image(info: String, file_size: u64, dimensions: Option<(u32, u32)>) -> Self {
        Self {
            preview_type: PreviewType::Image,
            content: info,
            file_size,
            metadata: Some(PreviewMetadata {
                extension: None,
                line_count: None,
                dimensions,
            }),
        }
    }

    /// Create an empty file preview.
    #[must_use]
    pub fn empty(file_size: u64) -> Self {
        Self {
            preview_type: PreviewType::Empty,
            content: "(empty file)".to_string(),
            file_size,
            metadata: None,
        }
    }

    /// Create an error preview.
    #[must_use]
    pub fn error(message: String) -> Self {
        Self {
            preview_type: PreviewType::Error,
            content: message,
            file_size: 0,
            metadata: None,
        }
    }
}

/// Preview a file, automatically detecting the appropriate preview type.
///
/// This function determines whether a file is text, binary, or an image,
/// and returns the appropriate preview content.
///
/// # Arguments
///
/// * `path` - Path to the file to preview
///
/// # Returns
///
/// Returns `PreviewContent` with the preview data.
///
/// # Errors
///
/// Returns `PreviewError` if the file cannot be read.
///
/// # Example
///
/// ```no_run
/// use rustdupe::actions::preview::preview_file;
/// use std::path::Path;
///
/// let result = preview_file(Path::new("example.txt"));
/// if let Ok(preview) = result {
///     println!("{}", preview.content);
/// }
/// ```
pub fn preview_file(path: &Path) -> Result<PreviewContent, PreviewError> {
    // Get file metadata
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(PreviewError::NotFound(path.display().to_string()));
        }
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
            return Err(PreviewError::PermissionDenied(path.display().to_string()));
        }
        Err(e) => return Err(PreviewError::Io(e)),
    };

    let file_size = metadata.len();

    // Handle empty files
    if file_size == 0 {
        return Ok(PreviewContent::empty(0));
    }

    // Check file extension for type hints
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    // Check if it's an image file
    if let Some(ref ext) = extension {
        if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
            return preview_image(path, file_size, ext);
        }
    }

    // Check if it's likely a text file based on extension
    let likely_text = extension
        .as_ref()
        .is_some_and(|ext| TEXT_EXTENSIONS.contains(&ext.as_str()));

    // Try to preview as text if extension suggests it, otherwise detect
    if likely_text {
        preview_text(path, file_size)
    } else {
        // Detect binary vs text by sampling content
        detect_and_preview(path, file_size)
    }
}

/// Preview a text file, returning the first N lines.
///
/// # Arguments
///
/// * `path` - Path to the file
/// * `file_size` - File size in bytes
///
/// # Returns
///
/// Text preview with first 50 lines or error if not readable as text.
fn preview_text(path: &Path, file_size: u64) -> Result<PreviewContent, PreviewError> {
    let file = open_file(path)?;
    let reader = BufReader::new(file);

    let mut lines = Vec::new();
    let mut total_bytes = 0;

    for line_result in reader.lines() {
        match line_result {
            Ok(line) => {
                total_bytes += line.len() + 1;

                if lines.len() < MAX_PREVIEW_LINES && total_bytes <= MAX_PREVIEW_BYTES {
                    lines.push(line);
                } else if lines.len() >= MAX_PREVIEW_LINES {
                    lines.push(format!("... ({} more lines)", "(truncated)"));
                    break;
                } else {
                    lines.push("... (content truncated at 4KB)".to_string());
                    break;
                }
            }
            Err(_e) => {
                // Contains binary data, switch to binary preview
                if lines.is_empty() {
                    return preview_binary(path, file_size);
                }
                lines.push("... (binary data follows)".to_string());
                break;
            }
        }
    }

    if lines.is_empty() {
        return Ok(PreviewContent::empty(file_size));
    }

    let shown_lines = lines.len();
    Ok(PreviewContent::text(
        lines.join("\n"),
        file_size,
        shown_lines,
    ))
}

/// Preview a binary file with hex dump.
///
/// # Arguments
///
/// * `path` - Path to the file
/// * `file_size` - File size in bytes
///
/// # Returns
///
/// Hex dump of first 256 bytes.
fn preview_binary(path: &Path, file_size: u64) -> Result<PreviewContent, PreviewError> {
    let mut file = open_file(path)?;

    // Read first 256 bytes
    let mut buffer = vec![0u8; HEX_DUMP_BYTES.min(file_size as usize)];
    let bytes_read = file.read(&mut buffer)?;
    buffer.truncate(bytes_read);

    // Format as hex dump
    let hex_dump = format_hex_dump(&buffer);

    Ok(PreviewContent::binary(hex_dump, file_size))
}

/// Preview an image file with metadata.
///
/// # Arguments
///
/// * `path` - Path to the file
/// * `file_size` - File size in bytes
/// * `extension` - File extension
///
/// # Returns
///
/// Image metadata preview.
fn preview_image(
    path: &Path,
    file_size: u64,
    extension: &str,
) -> Result<PreviewContent, PreviewError> {
    // Format file size
    let size_str = format_file_size(file_size);

    // Try to read image dimensions (basic detection for common formats)
    let dimensions = detect_image_dimensions(path, extension);

    let mut info = format!(
        "Image File\n\
         Format: {}\n\
         Size: {}",
        extension.to_uppercase(),
        size_str
    );

    if let Some((w, h)) = dimensions {
        info.push_str(&format!("\nDimensions: {} x {} pixels", w, h));
    }

    info.push_str(&format!("\nPath: {}", path.display()));

    Ok(PreviewContent::image(info, file_size, dimensions))
}

/// Detect whether file is text or binary and preview accordingly.
fn detect_and_preview(path: &Path, file_size: u64) -> Result<PreviewContent, PreviewError> {
    let mut file = open_file(path)?;

    // Read first 512 bytes to detect binary content
    let mut buffer = vec![0u8; BINARY_DETECT_BYTES.min(file_size as usize)];
    let bytes_read = file.read(&mut buffer)?;
    buffer.truncate(bytes_read);

    // Check for binary content (null bytes or high proportion of non-printable chars)
    if is_binary(&buffer) {
        // Reset file position and do binary preview
        file.seek(SeekFrom::Start(0))?;
        drop(file);
        preview_binary(path, file_size)
    } else {
        // Reset file position and do text preview
        file.seek(SeekFrom::Start(0))?;
        drop(file);
        preview_text(path, file_size)
    }
}

/// Check if data appears to be binary.
fn is_binary(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }

    // Null bytes almost always indicate binary
    if data.contains(&0) {
        return true;
    }

    // Count non-text characters
    let non_text_count = data
        .iter()
        .filter(|&&b| {
            // Allow printable ASCII, tabs, newlines, carriage returns
            !matches!(b, 0x09 | 0x0A | 0x0D | 0x20..=0x7E | 0x80..=0xFF)
        })
        .count();

    // If more than 10% are non-text, consider it binary
    non_text_count > data.len() / 10
}

/// Open a file with appropriate error handling.
fn open_file(path: &Path) -> Result<File, PreviewError> {
    match File::open(path) {
        Ok(f) => Ok(f),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            Err(PreviewError::NotFound(path.display().to_string()))
        }
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
            Err(PreviewError::PermissionDenied(path.display().to_string()))
        }
        Err(e) => Err(PreviewError::Io(e)),
    }
}

/// Format bytes as a hex dump with ASCII representation.
fn format_hex_dump(data: &[u8]) -> String {
    let mut output = String::new();
    let bytes_per_line = 16;

    for (i, chunk) in data.chunks(bytes_per_line).enumerate() {
        // Offset
        output.push_str(&format!("{:08X}  ", i * bytes_per_line));

        // Hex bytes
        for (j, byte) in chunk.iter().enumerate() {
            output.push_str(&format!("{:02X} ", byte));
            if j == 7 {
                output.push(' '); // Extra space in middle
            }
        }

        // Padding for incomplete last line
        let padding = bytes_per_line - chunk.len();
        for j in 0..padding {
            output.push_str("   ");
            if chunk.len() + j == 7 {
                output.push(' ');
            }
        }

        // ASCII representation
        output.push_str(" |");
        for byte in chunk {
            if byte.is_ascii_graphic() || *byte == b' ' {
                output.push(*byte as char);
            } else {
                output.push('.');
            }
        }
        output.push_str("|\n");
    }

    output
}

/// Format file size as human-readable string.
fn format_file_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        format!("{:.2} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else {
        format!("{} bytes", size)
    }
}

/// Detect image dimensions for common formats.
///
/// This is a simplified detection that reads the header bytes.
/// For full support, a dedicated image library would be needed.
fn detect_image_dimensions(path: &Path, extension: &str) -> Option<(u32, u32)> {
    let mut file = File::open(path).ok()?;
    let mut header = [0u8; 32];
    file.read_exact(&mut header).ok()?;

    match extension {
        "png" => detect_png_dimensions(&header),
        "jpg" | "jpeg" => detect_jpeg_dimensions(path),
        "gif" => detect_gif_dimensions(&header),
        "bmp" => detect_bmp_dimensions(&header),
        _ => None,
    }
}

/// Detect PNG dimensions from header.
fn detect_png_dimensions(header: &[u8]) -> Option<(u32, u32)> {
    // PNG signature: 89 50 4E 47 0D 0A 1A 0A
    if header.len() >= 24 && &header[0..8] == b"\x89PNG\r\n\x1a\n" {
        let width = u32::from_be_bytes([header[16], header[17], header[18], header[19]]);
        let height = u32::from_be_bytes([header[20], header[21], header[22], header[23]]);
        Some((width, height))
    } else {
        None
    }
}

/// Detect JPEG dimensions (requires reading more of the file).
fn detect_jpeg_dimensions(path: &Path) -> Option<(u32, u32)> {
    let mut file = File::open(path).ok()?;
    let mut buf = [0u8; 2];

    // Check JPEG signature
    file.read_exact(&mut buf).ok()?;
    if buf != [0xFF, 0xD8] {
        return None;
    }

    // Search for SOF marker (0xFF, 0xC0-0xCF excluding 0xC4, 0xC8, 0xCC)
    loop {
        // Find marker
        file.read_exact(&mut buf).ok()?;
        if buf[0] != 0xFF {
            return None;
        }

        // Skip padding
        while buf[1] == 0xFF {
            file.read_exact(&mut [0u8; 1]).ok()?;
            buf[1] = 0u8;
            let mut single = [0u8; 1];
            file.read_exact(&mut single).ok()?;
            buf[1] = single[0];
        }

        let marker = buf[1];

        // Check for SOF markers (Start of Frame)
        if (0xC0..=0xCF).contains(&marker) && marker != 0xC4 && marker != 0xC8 && marker != 0xCC {
            // Read length (2 bytes) + precision (1 byte) + height (2 bytes) + width (2 bytes)
            let mut data = [0u8; 7];
            file.read_exact(&mut data).ok()?;

            let height = u16::from_be_bytes([data[3], data[4]]) as u32;
            let width = u16::from_be_bytes([data[5], data[6]]) as u32;
            return Some((width, height));
        }

        // Skip this segment
        let mut len_buf = [0u8; 2];
        file.read_exact(&mut len_buf).ok()?;
        let len = u16::from_be_bytes(len_buf) as i64 - 2;
        if len > 0 {
            file.seek(SeekFrom::Current(len)).ok()?;
        }

        // Safety limit
        if file.stream_position().ok()? > 100_000 {
            return None;
        }
    }
}

/// Detect GIF dimensions from header.
fn detect_gif_dimensions(header: &[u8]) -> Option<(u32, u32)> {
    // GIF signature: GIF87a or GIF89a
    if header.len() >= 10 && (&header[0..3] == b"GIF") {
        let width = u16::from_le_bytes([header[6], header[7]]) as u32;
        let height = u16::from_le_bytes([header[8], header[9]]) as u32;
        Some((width, height))
    } else {
        None
    }
}

/// Detect BMP dimensions from header.
fn detect_bmp_dimensions(header: &[u8]) -> Option<(u32, u32)> {
    // BMP signature: BM
    if header.len() >= 26 && &header[0..2] == b"BM" {
        let width =
            i32::from_le_bytes([header[18], header[19], header[20], header[21]]).unsigned_abs();
        let height =
            i32::from_le_bytes([header[22], header[23], header[24], header[25]]).unsigned_abs();
        Some((width, height))
    } else {
        None
    }
}

/// Simple preview function for TUI integration.
///
/// This is a convenience wrapper that returns a plain string,
/// suitable for direct display in the TUI preview area.
///
/// # Arguments
///
/// * `path` - Path to the file to preview
///
/// # Returns
///
/// A string with the preview content, or an error message.
#[must_use]
pub fn preview_file_simple(path: &Path) -> String {
    match preview_file(path) {
        Ok(content) => content.content,
        Err(e) => format!("Preview error: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_preview_type_variants() {
        assert_ne!(PreviewType::Text, PreviewType::Binary);
        assert_ne!(PreviewType::Image, PreviewType::Empty);
        assert_ne!(PreviewType::Error, PreviewType::Text);
    }

    #[test]
    fn test_preview_content_text() {
        let content = PreviewContent::text("hello\nworld".to_string(), 100, 2);
        assert_eq!(content.preview_type, PreviewType::Text);
        assert_eq!(content.content, "hello\nworld");
        assert_eq!(content.file_size, 100);
        assert!(content.metadata.is_some());
    }

    #[test]
    fn test_preview_content_binary() {
        let content = PreviewContent::binary("00 01 02".to_string(), 256);
        assert_eq!(content.preview_type, PreviewType::Binary);
    }

    #[test]
    fn test_preview_content_empty() {
        let content = PreviewContent::empty(0);
        assert_eq!(content.preview_type, PreviewType::Empty);
        assert!(content.content.contains("empty"));
    }

    #[test]
    fn test_preview_content_error() {
        let content = PreviewContent::error("test error".to_string());
        assert_eq!(content.preview_type, PreviewType::Error);
    }

    #[test]
    fn test_preview_text_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Line 1").unwrap();
        writeln!(file, "Line 2").unwrap();
        writeln!(file, "Line 3").unwrap();

        let result = preview_file(file.path());
        assert!(result.is_ok());
        let preview = result.unwrap();
        assert_eq!(preview.preview_type, PreviewType::Text);
        assert!(preview.content.contains("Line 1"));
        assert!(preview.content.contains("Line 2"));
    }

    #[test]
    fn test_preview_empty_file() {
        let file = NamedTempFile::new().unwrap();
        // Don't write anything - file is empty

        let result = preview_file(file.path());
        assert!(result.is_ok());
        let preview = result.unwrap();
        assert_eq!(preview.preview_type, PreviewType::Empty);
    }

    #[test]
    fn test_preview_binary_file() {
        let mut file = NamedTempFile::new().unwrap();
        // Write binary content with null bytes
        file.write_all(&[0x00, 0x01, 0x02, 0x03, 0xFF, 0xFE])
            .unwrap();

        let result = preview_file(file.path());
        assert!(result.is_ok());
        let preview = result.unwrap();
        assert_eq!(preview.preview_type, PreviewType::Binary);
    }

    #[test]
    fn test_preview_nonexistent_file() {
        let result = preview_file(Path::new("/nonexistent/file.txt"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PreviewError::NotFound(_)));
    }

    #[test]
    fn test_is_binary_with_null() {
        assert!(is_binary(&[0x00, 0x01, 0x02]));
    }

    #[test]
    fn test_is_binary_text() {
        assert!(!is_binary(b"Hello, World!"));
    }

    #[test]
    fn test_is_binary_empty() {
        assert!(!is_binary(&[]));
    }

    #[test]
    fn test_format_hex_dump() {
        let data = b"Hello, World!";
        let dump = format_hex_dump(data);
        assert!(dump.contains("48 65 6C 6C")); // "Hell" in hex
        assert!(dump.contains("|Hello, World!|"));
    }

    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(100), "100 bytes");
        assert_eq!(format_file_size(1024), "1.00 KB");
        assert_eq!(format_file_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_file_size(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_detect_png_dimensions() {
        let header: [u8; 24] = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
            0x00, 0x00, 0x01, 0x00, // width = 256
            0x00, 0x00, 0x00, 0x80, // height = 128
        ];
        let dims = detect_png_dimensions(&header);
        assert_eq!(dims, Some((256, 128)));
    }

    #[test]
    fn test_detect_gif_dimensions() {
        let header: [u8; 10] = [
            0x47, 0x49, 0x46, 0x38, 0x39, 0x61, // GIF89a
            0x40, 0x01, // width = 320
            0xF0, 0x00, // height = 240
        ];
        let dims = detect_gif_dimensions(&header);
        assert_eq!(dims, Some((320, 240)));
    }

    #[test]
    fn test_detect_bmp_dimensions() {
        let mut header = [0u8; 26];
        header[0] = b'B';
        header[1] = b'M';
        // Width at offset 18-21 (little-endian i32)
        header[18] = 0x80; // 128
        header[19] = 0x00;
        header[20] = 0x00;
        header[21] = 0x00;
        // Height at offset 22-25 (little-endian i32)
        header[22] = 0x60; // 96
        header[23] = 0x00;
        header[24] = 0x00;
        header[25] = 0x00;

        let dims = detect_bmp_dimensions(&header);
        assert_eq!(dims, Some((128, 96)));
    }

    #[test]
    fn test_preview_file_simple() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Simple content").unwrap();

        let content = preview_file_simple(file.path());
        assert!(content.contains("Simple content"));
    }

    #[test]
    fn test_preview_file_simple_error() {
        let content = preview_file_simple(Path::new("/nonexistent/path"));
        assert!(content.contains("error") || content.contains("Error"));
    }

    #[test]
    fn test_preview_error_display() {
        let err = PreviewError::NotFound("test.txt".to_string());
        assert!(err.to_string().contains("not found"));

        let err = PreviewError::PermissionDenied("secret.txt".to_string());
        assert!(err.to_string().contains("permission denied"));
    }
}
