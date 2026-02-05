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

pub mod hardlink;
pub mod hasher;
pub mod path_utils;
pub mod walker;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

// Re-export main types
pub use hardlink::HardlinkTracker;
pub use hasher::{hash_to_hex, hex_to_hash, Hash, Hasher, PREHASH_SIZE};
pub use path_utils::{
    is_nfc, normalize_path_str, normalize_path_str_cow, normalize_pathbuf, path_key, paths_equal,
    paths_equal_normalized,
};
use regex::Regex;
pub use walker::{MultiWalker, Walker};

/// File categories for filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileCategory {
    /// Image files (jpg, png, etc.)
    Images,
    /// Video files (mp4, mkv, etc.)
    Videos,
    /// Audio files (mp3, wav, etc.)
    Audio,
    /// Document files (pdf, docx, etc.)
    Documents,
    /// Archive files (zip, tar, etc.)
    Archives,
}

impl FileCategory {
    /// Get the list of extensions for this category.
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            FileCategory::Images => &["jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "svg"],
            FileCategory::Videos => &["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm"],
            FileCategory::Audio => &["mp3", "wav", "flac", "m4a", "ogg", "wma"],
            FileCategory::Documents => &[
                "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "rtf", "odt", "ods",
                "odp",
            ],
            FileCategory::Archives => &["zip", "tar", "gz", "7z", "rar", "bz2", "xz"],
        }
    }
}

/// Metadata for a discovered file.
///
/// Contains all information needed for duplicate detection,
/// including path, size, modification time, and link status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
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
    /// Optional group name (set when using --group flag)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_name: Option<String>,
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
            group_name: None,
        }
    }

    /// Create a new FileEntry with a group name.
    #[must_use]
    pub fn with_group(path: PathBuf, size: u64, modified: SystemTime, group_name: String) -> Self {
        Self {
            path,
            size,
            modified,
            is_symlink: false,
            is_hardlink: false,
            group_name: Some(group_name),
        }
    }

    /// Set the group name for this entry.
    pub fn set_group_name(&mut self, name: String) {
        self.group_name = Some(name);
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

    /// Only include files modified after this time.
    pub newer_than: Option<SystemTime>,

    /// Only include files modified before this time.
    pub older_than: Option<SystemTime>,

    /// Glob patterns to ignore (gitignore-style).
    /// These are applied in addition to any .gitignore files.
    pub ignore_patterns: Vec<String>,

    /// Regex patterns to include (filename must match at least one).
    pub regex_include: Vec<Regex>,

    /// Regex patterns to exclude (filename must not match any).
    pub regex_exclude: Vec<Regex>,

    /// File categories to include (if empty, all types are included).
    pub file_categories: Vec<FileCategory>,
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
    /// * `newer_than` - Only include files modified after this time
    /// * `older_than` - Only include files modified before this time
    /// * `ignore_patterns` - Glob patterns to ignore
    #[must_use]
    pub fn new(
        follow_symlinks: bool,
        skip_hidden: bool,
        min_size: Option<u64>,
        max_size: Option<u64>,
        newer_than: Option<SystemTime>,
        older_than: Option<SystemTime>,
        ignore_patterns: Vec<String>,
    ) -> Self {
        Self {
            follow_symlinks,
            skip_hidden,
            min_size,
            max_size,
            newer_than,
            older_than,
            ignore_patterns,
            regex_include: Vec::new(),
            regex_exclude: Vec::new(),
            file_categories: Vec::new(),
        }
    }

    /// Set whether to follow symbolic links.
    #[must_use]
    pub fn with_follow_symlinks(mut self, follow: bool) -> Self {
        self.follow_symlinks = follow;
        self
    }

    /// Set whether to skip hidden files.
    #[must_use]
    pub fn with_skip_hidden(mut self, skip: bool) -> Self {
        self.skip_hidden = skip;
        self
    }

    /// Set minimum file size filter.
    #[must_use]
    pub fn with_min_size(mut self, size: Option<u64>) -> Self {
        self.min_size = size;
        self
    }

    /// Set maximum file size filter.
    #[must_use]
    pub fn with_max_size(mut self, size: Option<u64>) -> Self {
        self.max_size = size;
        self
    }

    /// Set newer-than date filter.
    #[must_use]
    pub fn with_newer_than(mut self, time: Option<SystemTime>) -> Self {
        self.newer_than = time;
        self
    }

    /// Set older-than date filter.
    #[must_use]
    pub fn with_older_than(mut self, time: Option<SystemTime>) -> Self {
        self.older_than = time;
        self
    }

    /// Set glob patterns to ignore.
    #[must_use]
    pub fn with_patterns(mut self, patterns: Vec<String>) -> Self {
        self.ignore_patterns = patterns;
        self
    }

    /// Set regex include patterns.
    #[must_use]
    pub fn with_regex_include(mut self, regexes: Vec<Regex>) -> Self {
        self.regex_include = regexes;
        self
    }

    /// Set regex exclude patterns.
    #[must_use]
    pub fn with_regex_exclude(mut self, regexes: Vec<Regex>) -> Self {
        self.regex_exclude = regexes;
        self
    }

    /// Set file categories to include.
    #[must_use]
    pub fn with_file_categories(mut self, categories: Vec<FileCategory>) -> Self {
        self.file_categories = categories;
        self
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
            None,
            None,
            vec!["*.tmp".to_string()],
        );

        assert!(config.follow_symlinks);
        assert!(config.skip_hidden);
        assert_eq!(config.min_size, Some(1024));
        assert_eq!(config.max_size, Some(1_000_000));
        assert!(config.newer_than.is_none());
        assert!(config.older_than.is_none());
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
