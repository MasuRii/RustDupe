//! Scanner module for directory traversal and file hashing.
//!
//! This module provides functionality for:
//! - Parallel directory walking using jwalk
//! - Content hashing with BLAKE3
//! - Hardlink detection
//! - Unicode path normalization
//!
//! # Architecture
//!
//! The scanner is divided into submodules:
//! - [`walker`]: Directory traversal and file discovery
//! - [`hasher`]: BLAKE3 file hashing (streaming)
//!
//! # Example
//!
//! ```no_run
//! use rustdupe::scanner::{Walker, WalkerConfig, FileEntry};
//! use std::path::Path;
//!
//! // Configure the walker
//! let config = WalkerConfig {
//!     min_size: Some(1024),  // Skip files under 1KB
//!     skip_hidden: true,     // Skip hidden files
//!     ..Default::default()
//! };
//!
//! // Walk the directory
//! let walker = Walker::new(Path::new("."), config);
//! for entry in walker.walk() {
//!     match entry {
//!         Ok(file) => println!("{}: {} bytes", file.path.display(), file.size),
//!         Err(e) => eprintln!("Warning: {}", e),
//!     }
//! }
//! ```

pub mod hasher;
pub mod walker;

use std::path::PathBuf;
use std::time::SystemTime;

// Re-export main types
pub use hasher::{hash_to_hex, hex_to_hash, Hash, Hasher, PREHASH_SIZE};
pub use walker::Walker;

/// Metadata for a discovered file.
///
/// Contains all information needed for duplicate detection,
/// including path, size, modification time, and link status.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Absolute path to the file
    pub path: PathBuf,
    /// File size in bytes
    pub size: u64,
    /// Last modification time
    pub modified: SystemTime,
    /// Whether this file is a symbolic link
    pub is_symlink: bool,
    /// Whether this file is a hardlink to a previously seen file
    pub is_hardlink: bool,
}

impl FileEntry {
    /// Create a new FileEntry.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file
    /// * `size` - File size in bytes
    /// * `modified` - Last modification time
    #[must_use]
    pub fn new(path: PathBuf, size: u64, modified: SystemTime) -> Self {
        Self {
            path,
            size,
            modified,
            is_symlink: false,
            is_hardlink: false,
        }
    }
}

/// Configuration for directory walking.
///
/// Controls filtering, symlink handling, and other walk behavior.
#[derive(Debug, Clone, Default)]
pub struct WalkerConfig {
    /// Follow symbolic links during traversal.
    /// Warning: May cause infinite loops with symlink cycles.
    pub follow_symlinks: bool,

    /// Skip hidden files and directories (names starting with `.`).
    pub skip_hidden: bool,

    /// Minimum file size to include (in bytes).
    /// Files smaller than this are skipped.
    pub min_size: Option<u64>,

    /// Maximum file size to include (in bytes).
    /// Files larger than this are skipped.
    pub max_size: Option<u64>,

    /// Glob patterns to ignore (gitignore-style).
    /// These are applied in addition to any .gitignore files.
    pub ignore_patterns: Vec<String>,
}

impl WalkerConfig {
    /// Create a new configuration from CLI arguments.
    ///
    /// # Arguments
    ///
    /// * `follow_symlinks` - Whether to follow symbolic links
    /// * `skip_hidden` - Whether to skip hidden files
    /// * `min_size` - Minimum file size filter
    /// * `max_size` - Maximum file size filter
    /// * `ignore_patterns` - Glob patterns to ignore
    #[must_use]
    pub fn new(
        follow_symlinks: bool,
        skip_hidden: bool,
        min_size: Option<u64>,
        max_size: Option<u64>,
        ignore_patterns: Vec<String>,
    ) -> Self {
        Self {
            follow_symlinks,
            skip_hidden,
            min_size,
            max_size,
            ignore_patterns,
        }
    }
}

/// Errors that can occur during directory scanning.
#[derive(thiserror::Error, Debug)]
pub enum ScanError {
    /// Permission was denied when accessing a file or directory.
    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),

    /// The specified path was not found.
    #[error("Path not found: {0}")]
    NotFound(PathBuf),

    /// The specified path is not a directory.
    #[error("Not a directory: {0}")]
    NotADirectory(PathBuf),

    /// An I/O error occurred while accessing a file.
    #[error("I/O error for {path}: {source}")]
    Io {
        /// Path where the error occurred
        path: PathBuf,
        /// The underlying I/O error
        #[source]
        source: std::io::Error,
    },
}

/// Errors that can occur during file hashing.
#[derive(thiserror::Error, Debug)]
pub enum HashError {
    /// The specified file was not found.
    #[error("File not found: {0}")]
    NotFound(PathBuf),

    /// Permission was denied when reading the file.
    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),

    /// An I/O error occurred while reading the file.
    #[error("I/O error for {path}: {source}")]
    Io {
        /// Path where the error occurred
        path: PathBuf,
        /// The underlying I/O error
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_entry_new() {
        let entry = FileEntry::new(PathBuf::from("/test/file.txt"), 1024, SystemTime::now());

        assert_eq!(entry.path, PathBuf::from("/test/file.txt"));
        assert_eq!(entry.size, 1024);
        assert!(!entry.is_symlink);
        assert!(!entry.is_hardlink);
    }

    #[test]
    fn test_walker_config_default() {
        let config = WalkerConfig::default();

        assert!(!config.follow_symlinks);
        assert!(!config.skip_hidden);
        assert!(config.min_size.is_none());
        assert!(config.max_size.is_none());
        assert!(config.ignore_patterns.is_empty());
    }

    #[test]
    fn test_walker_config_new() {
        let config = WalkerConfig::new(
            true,
            true,
            Some(1024),
            Some(1_000_000),
            vec!["*.tmp".to_string()],
        );

        assert!(config.follow_symlinks);
        assert!(config.skip_hidden);
        assert_eq!(config.min_size, Some(1024));
        assert_eq!(config.max_size, Some(1_000_000));
        assert_eq!(config.ignore_patterns, vec!["*.tmp".to_string()]);
    }

    #[test]
    fn test_scan_error_display() {
        let err = ScanError::PermissionDenied(PathBuf::from("/test"));
        assert_eq!(err.to_string(), "Permission denied: /test");

        let err = ScanError::NotFound(PathBuf::from("/missing"));
        assert_eq!(err.to_string(), "Path not found: /missing");

        let err = ScanError::NotADirectory(PathBuf::from("/file.txt"));
        assert_eq!(err.to_string(), "Not a directory: /file.txt");
    }

    #[test]
    fn test_hash_error_display() {
        let err = HashError::NotFound(PathBuf::from("/test"));
        assert_eq!(err.to_string(), "File not found: /test");

        let err = HashError::PermissionDenied(PathBuf::from("/secret"));
        assert_eq!(err.to_string(), "Permission denied: /secret");
    }
}
