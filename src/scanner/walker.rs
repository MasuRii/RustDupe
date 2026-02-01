//! Directory walker implementation using jwalk for parallel traversal.
//!
//! # Overview
//!
//! This module provides the [`Walker`] struct for efficiently traversing
//! directories and collecting file metadata for duplicate detection.
//! It uses [`jwalk`] for parallel directory walking (4x faster than walkdir).
//!
//! # Features
//!
//! - Parallel directory traversal using rayon thread pool
//! - Configurable symlink following with cycle detection
//! - Gitignore-style pattern matching via the `ignore` crate
//! - Size filtering (min/max)
//! - Hidden file filtering
//! - Hardlink detection via [`HardlinkTracker`]
//! - Graceful shutdown via atomic flag
//!
//! # Example
//!
//! ```no_run
//! use rustdupe::scanner::{Walker, WalkerConfig};
//! use std::path::Path;
//!
//! let config = WalkerConfig {
//!     min_size: Some(1024),  // Skip files under 1KB
//!     skip_hidden: true,
//!     ..Default::default()
//! };
//!
//! let walker = Walker::new(Path::new("/home/user/Downloads"), config);
//! for entry in walker.walk() {
//!     match entry {
//!         Ok(file) => println!("{}: {} bytes", file.path.display(), file.size),
//!         Err(e) => eprintln!("Warning: {}", e),
//!     }
//! }
//! ```

use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use jwalk::WalkDir;

use super::hardlink::HardlinkTracker;
use super::{FileEntry, ScanError, WalkerConfig};

/// Directory walker for parallel file discovery.
///
/// Uses jwalk for efficient parallel traversal of directory trees.
/// Supports filtering by size, patterns, and various file attributes.
#[derive(Debug)]
pub struct Walker {
    /// Root path to walk
    root: PathBuf,
    /// Walker configuration
    config: WalkerConfig,
    /// Optional shutdown flag for graceful termination
    shutdown_flag: Option<Arc<AtomicBool>>,
}

impl Walker {
    /// Create a new walker for the given path.
    ///
    /// # Arguments
    ///
    /// * `path` - Root directory to scan
    /// * `config` - Walker configuration options
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rustdupe::scanner::{Walker, WalkerConfig};
    /// use std::path::Path;
    ///
    /// let walker = Walker::new(Path::new("."), WalkerConfig::default());
    /// ```
    #[must_use]
    pub fn new(path: &Path, config: WalkerConfig) -> Self {
        Self {
            root: path.to_path_buf(),
            config,
            shutdown_flag: None,
        }
    }

    /// Set the shutdown flag for graceful termination.
    ///
    /// When the flag is set to `true`, the walker will stop iteration
    /// as soon as possible. This allows for clean Ctrl+C handling.
    ///
    /// # Arguments
    ///
    /// * `flag` - Atomic boolean flag shared across threads
    #[must_use]
    pub fn with_shutdown_flag(mut self, flag: Arc<AtomicBool>) -> Self {
        self.shutdown_flag = Some(flag);
        self
    }

    /// Check if shutdown has been requested.
    fn is_shutdown_requested(&self) -> bool {
        self.shutdown_flag
            .as_ref()
            .is_some_and(|f| f.load(Ordering::SeqCst))
    }

    /// Build gitignore matcher from config patterns and .gitignore file.
    fn build_gitignore(&self) -> Option<Gitignore> {
        let mut builder = GitignoreBuilder::new(&self.root);

        // Add local .gitignore if it exists
        let gitignore_path = self.root.join(".gitignore");
        if gitignore_path.exists() {
            if let Some(e) = builder.add(&gitignore_path) {
                log::warn!(
                    "Failed to load .gitignore from {}: {}",
                    gitignore_path.display(),
                    e
                );
            } else {
                log::debug!("Loaded .gitignore from {}", gitignore_path.display());
            }
        }

        // Add custom patterns from config
        for pattern in &self.config.ignore_patterns {
            if let Err(e) = builder.add_line(None, pattern) {
                log::warn!("Invalid ignore pattern '{}': {}", pattern, e);
            }
        }

        match builder.build() {
            Ok(gitignore) => {
                if gitignore.is_empty() {
                    None
                } else {
                    Some(gitignore)
                }
            }
            Err(e) => {
                log::warn!("Failed to build ignore patterns: {}", e);
                None
            }
        }
    }

    /// Check if a path should be ignored based on configured patterns.
    fn should_ignore(&self, path: &Path, is_dir: bool, gitignore: &Option<Gitignore>) -> bool {
        if let Some(gi) = gitignore {
            // Gitignore matching typically expects relative paths from the root
            // and uses forward slashes even on Windows.
            let relative_path = path.strip_prefix(&self.root).unwrap_or(path);

            // The ignore crate's Gitignore::matched() works best when we check
            // the path and all its parents, or at least ensure directory patterns
            // match correctly.
            let path_str = relative_path.to_string_lossy();
            let normalized_path = if cfg!(windows) {
                path_str.replace('\\', "/")
            } else {
                path_str.into_owned()
            };

            gi.matched(normalized_path, is_dir).is_ignore()
        } else {
            false
        }
    }

    /// Check if a file passes size filters.
    fn passes_size_filter(&self, size: u64) -> bool {
        if let Some(min) = self.config.min_size {
            if size < min {
                return false;
            }
        }
        if let Some(max) = self.config.max_size {
            if size > max {
                return false;
            }
        }
        true
    }

    /// Check if a file passes date filters.
    fn passes_date_filter(&self, modified: SystemTime) -> bool {
        if let Some(newer_than) = self.config.newer_than {
            if modified < newer_than {
                return false;
            }
        }
        if let Some(older_than) = self.config.older_than {
            if modified > older_than {
                return false;
            }
        }
        true
    }

    /// Check if a file passes regex filters.
    fn passes_regex_filter(&self, path: &Path) -> bool {
        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();

        // If include patterns are specified, at least one must match
        if !self.config.regex_include.is_empty() {
            let mut matched = false;
            for re in &self.config.regex_include {
                if re.is_match(&filename) {
                    matched = true;
                    break;
                }
            }
            if !matched {
                return false;
            }
        }

        // If exclude patterns are specified, none must match
        for re in &self.config.regex_exclude {
            if re.is_match(&filename) {
                return false;
            }
        }

        true
    }

    /// Check if a file passes file type filters.
    fn passes_file_type_filter(&self, path: &Path) -> bool {
        if self.config.file_categories.is_empty() {
            return true;
        }

        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        for category in &self.config.file_categories {
            if category.extensions().contains(&extension.as_str()) {
                return true;
            }
        }

        false
    }

    /// Walk the directory tree, yielding file entries.
    ///
    /// Returns an iterator over [`FileEntry`] results. Errors are yielded
    /// as [`ScanError`] values rather than stopping iteration.
    ///
    /// # Performance
    ///
    /// Uses parallel directory reading via jwalk. Typically 4x faster
    /// than single-threaded walkdir for large directories.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rustdupe::scanner::{Walker, WalkerConfig};
    /// use std::path::Path;
    ///
    /// let walker = Walker::new(Path::new("."), WalkerConfig::default());
    /// let files: Vec<_> = walker.walk().filter_map(Result::ok).collect();
    /// println!("Found {} files", files.len());
    /// ```
    pub fn walk(&self) -> impl Iterator<Item = Result<FileEntry, ScanError>> + '_ {
        let gitignore = self.build_gitignore();
        let mut hardlink_tracker = HardlinkTracker::new();

        // Configure jwalk
        let walk_dir = WalkDir::new(&self.root)
            .follow_links(self.config.follow_symlinks)
            .skip_hidden(self.config.skip_hidden)
            .process_read_dir(move |_depth, _path, _read_dir_state, children| {
                // Sort children for deterministic output
                children.sort_by(|a, b| match (a, b) {
                    (Ok(a), Ok(b)) => a.file_name().cmp(b.file_name()),
                    (Ok(_), Err(_)) => std::cmp::Ordering::Less,
                    (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
                    (Err(_), Err(_)) => std::cmp::Ordering::Equal,
                });
            });

        walk_dir.into_iter().filter_map(move |entry_result| {
            // Check shutdown flag periodically
            if self.is_shutdown_requested() {
                log::debug!("Walker: Shutdown requested, stopping iteration");
                return None;
            }

            match entry_result {
                Ok(entry) => {
                    let path = entry.path();

                    // Skip the root directory itself
                    if path == self.root {
                        return None;
                    }

                    // Get file type (jwalk returns FileType directly)
                    let file_type = entry.file_type();

                    // Skip directories (we only want files)
                    if file_type.is_dir() {
                        // But still check if we should ignore this directory
                        if self.should_ignore(&path, true, &gitignore) {
                            log::trace!("Ignoring directory: {}", path.display());
                        }
                        return None;
                    }

                    // Check ignore patterns
                    if self.should_ignore(&path, false, &gitignore) {
                        log::trace!("Ignoring file: {}", path.display());
                        return None;
                    }

                    // Handle symlinks
                    let is_symlink = file_type.is_symlink();
                    if is_symlink && !self.config.follow_symlinks {
                        log::trace!("Skipping symlink: {}", path.display());
                        return None;
                    }

                    // Get metadata (follow symlinks if configured)
                    let metadata = if self.config.follow_symlinks {
                        std::fs::metadata(&path)
                    } else {
                        std::fs::symlink_metadata(&path)
                    };

                    let metadata = match metadata {
                        Ok(m) => m,
                        Err(e) => {
                            return Some(self.handle_io_error(&path, e));
                        }
                    };

                    // Skip if not a regular file after following symlink
                    if !metadata.is_file() {
                        return None;
                    }

                    // Process the file entry
                    self.process_file_entry(
                        path,
                        metadata,
                        is_symlink,
                        &mut hardlink_tracker,
                        &gitignore,
                    )
                }
                Err(e) => {
                    // Convert jwalk error to ScanError
                    let path = e
                        .path()
                        .map_or_else(|| self.root.clone(), std::borrow::ToOwned::to_owned);
                    Some(self.handle_jwalk_error(path, e))
                }
            }
        })
    }

    /// Process a file entry and create a FileEntry if valid.
    fn process_file_entry(
        &self,
        path: PathBuf,
        metadata: Metadata,
        is_symlink: bool,
        hardlink_tracker: &mut HardlinkTracker,
        _gitignore: &Option<Gitignore>,
    ) -> Option<Result<FileEntry, ScanError>> {
        let size = metadata.len();

        // Skip empty files with a warning (they all hash the same)
        if size == 0 {
            log::debug!("Skipping empty file: {}", path.display());
            return None;
        }

        // Apply size filters
        if !self.passes_size_filter(size) {
            log::trace!(
                "Skipping file due to size filter ({}): {}",
                size,
                path.display()
            );
            return None;
        }

        // Get modification time
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);

        // Apply date filters
        if !self.passes_date_filter(modified) {
            log::trace!("Skipping file due to date filter: {}", path.display());
            return None;
        }

        // Apply regex filters
        if !self.passes_regex_filter(&path) {
            log::trace!("Skipping file due to regex filter: {}", path.display());
            return None;
        }

        // Apply file type filters
        if !self.passes_file_type_filter(&path) {
            log::trace!("Skipping file due to file type filter: {}", path.display());
            return None;
        }

        // Check for hardlinks using the tracker
        if hardlink_tracker.is_hardlink(&metadata) {
            log::debug!("Skipping hardlink: {}", path.display());
            return None;
        }

        Some(Ok(FileEntry {
            path,
            size,
            modified,
            is_symlink,
            is_hardlink: false,
        }))
    }

    /// Handle I/O errors during file access.
    fn handle_io_error(&self, path: &Path, error: std::io::Error) -> Result<FileEntry, ScanError> {
        use std::io::ErrorKind;

        match error.kind() {
            ErrorKind::PermissionDenied => {
                log::warn!("Permission denied: {}", path.display());
                Err(ScanError::PermissionDenied(path.to_path_buf()))
            }
            ErrorKind::NotFound => {
                log::debug!("File not found (may have been deleted): {}", path.display());
                Err(ScanError::NotFound(path.to_path_buf()))
            }
            _ => {
                log::warn!("I/O error for {}: {}", path.display(), error);
                Err(ScanError::Io {
                    path: path.to_path_buf(),
                    source: error,
                })
            }
        }
    }

    /// Handle jwalk errors.
    fn handle_jwalk_error(
        &self,
        path: PathBuf,
        error: jwalk::Error,
    ) -> Result<FileEntry, ScanError> {
        // Extract the underlying I/O error if available
        log::warn!("Walker error for {}: {}", path.display(), error);
        Err(ScanError::Io {
            path,
            source: std::io::Error::other(error.to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;

    /// Create a test directory with some files.
    fn create_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();

        // Create some test files
        let file1 = dir.path().join("file1.txt");
        let mut f = File::create(&file1).unwrap();
        writeln!(f, "Hello, world!").unwrap();

        let file2 = dir.path().join("file2.txt");
        let mut f = File::create(&file2).unwrap();
        writeln!(f, "Another file").unwrap();

        // Create a subdirectory with a file
        let subdir = dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();

        let file3 = subdir.join("nested.txt");
        let mut f = File::create(&file3).unwrap();
        writeln!(f, "Nested file content").unwrap();

        dir
    }

    #[test]
    fn test_walker_finds_files() {
        let dir = create_test_dir();
        let walker = Walker::new(dir.path(), WalkerConfig::default());

        let files: Vec<_> = walker.walk().filter_map(Result::ok).collect();

        assert_eq!(files.len(), 3);

        // Verify all files are regular files with non-zero size
        for file in &files {
            assert!(file.size > 0);
            assert!(file.path.exists());
            assert!(!file.is_symlink);
        }
    }

    #[test]
    fn test_walker_min_size_filter() {
        let dir = create_test_dir();

        // Create a very small file (1 byte)
        let tiny_file = dir.path().join("tiny.txt");
        let mut f = File::create(&tiny_file).unwrap();
        f.write_all(b"X").unwrap();

        let config = WalkerConfig {
            min_size: Some(10), // Minimum 10 bytes
            ..Default::default()
        };
        let walker = Walker::new(dir.path(), config);

        let files: Vec<_> = walker.walk().filter_map(Result::ok).collect();

        // The tiny file should be filtered out
        for file in &files {
            assert!(
                file.size >= 10,
                "File {} has size {}",
                file.path.display(),
                file.size
            );
        }
    }

    #[test]
    fn test_walker_max_size_filter() {
        let dir = create_test_dir();

        // Create a larger file
        let large_file = dir.path().join("large.txt");
        let mut f = File::create(&large_file).unwrap();
        for _ in 0..1000 {
            writeln!(f, "This is a line of text to make the file larger.").unwrap();
        }

        let config = WalkerConfig {
            max_size: Some(100), // Maximum 100 bytes
            ..Default::default()
        };
        let walker = Walker::new(dir.path(), config);

        let files: Vec<_> = walker.walk().filter_map(Result::ok).collect();

        // All files should be under 100 bytes
        for file in &files {
            assert!(
                file.size <= 100,
                "File {} has size {}",
                file.path.display(),
                file.size
            );
        }
    }

    #[test]
    fn test_walker_skip_empty_files() {
        let dir = create_test_dir();

        // Create an empty file
        File::create(dir.path().join("empty.txt")).unwrap();

        let walker = Walker::new(dir.path(), WalkerConfig::default());

        let files: Vec<_> = walker.walk().filter_map(Result::ok).collect();

        // Empty file should be skipped
        for file in &files {
            assert!(file.size > 0);
        }
    }

    #[test]
    fn test_walker_skip_hidden_files() {
        let dir = create_test_dir();

        // Create a hidden file
        let hidden_file = dir.path().join(".hidden");
        let mut f = File::create(&hidden_file).unwrap();
        writeln!(f, "Hidden content").unwrap();

        let config = WalkerConfig {
            skip_hidden: true,
            ..Default::default()
        };
        let walker = Walker::new(dir.path(), config);

        let files: Vec<_> = walker.walk().filter_map(Result::ok).collect();

        // Hidden file should be skipped
        for file in &files {
            assert!(!file
                .path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with('.'));
        }
    }

    #[test]
    fn test_walker_ignore_patterns() {
        let dir = create_test_dir();

        // Create files matching ignore patterns
        let tmp_file = dir.path().join("temp.tmp");
        let mut f = File::create(&tmp_file).unwrap();
        writeln!(f, "Temporary file").unwrap();

        let log_file = dir.path().join("debug.log");
        let mut f = File::create(&log_file).unwrap();
        writeln!(f, "Log content").unwrap();

        let config = WalkerConfig {
            ignore_patterns: vec!["*.tmp".to_string(), "*.log".to_string()],
            ..Default::default()
        };
        let walker = Walker::new(dir.path(), config);

        let files: Vec<_> = walker.walk().filter_map(Result::ok).collect();

        // Ignored files should be skipped
        for file in &files {
            let name = file.path.file_name().unwrap().to_str().unwrap();
            assert!(!name.ends_with(".tmp"), "Should skip .tmp files");
            assert!(!name.ends_with(".log"), "Should skip .log files");
        }
    }

    #[test]
    fn test_walker_date_filters() {
        use chrono::{TimeZone, Utc};
        let dir = TempDir::new().unwrap();

        // Create a file modified in the past
        let past_file = dir.path().join("past.txt");
        let mut f = File::create(&past_file).unwrap();
        writeln!(f, "past content").unwrap();
        let past_time = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        filetime::set_file_mtime(
            &past_file,
            filetime::FileTime::from_system_time(past_time.into()),
        )
        .unwrap();

        // Create a file modified recently
        let recent_file = dir.path().join("recent.txt");
        let mut f = File::create(&recent_file).unwrap();
        writeln!(f, "recent content").unwrap();
        let recent_time = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        filetime::set_file_mtime(
            &recent_file,
            filetime::FileTime::from_system_time(recent_time.into()),
        )
        .unwrap();

        // Test newer_than
        let config = WalkerConfig::default().with_newer_than(Some(
            Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap().into(),
        ));
        let walker = Walker::new(dir.path(), config);
        let files: Vec<_> = walker.walk().filter_map(Result::ok).collect();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path.file_name().unwrap(), "recent.txt");

        // Test older_than
        let config = WalkerConfig::default().with_older_than(Some(
            Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap().into(),
        ));
        let walker = Walker::new(dir.path(), config);
        let files: Vec<_> = walker.walk().filter_map(Result::ok).collect();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path.file_name().unwrap(), "past.txt");
    }

    #[test]
    fn test_walker_combined_date_filters() {
        use chrono::{TimeZone, Utc};
        let dir = TempDir::new().unwrap();

        // past: 2020-01-01
        // mid: 2023-01-01
        // recent: 2026-01-01

        let past_file = dir.path().join("past.txt");
        let mut f = File::create(&past_file).unwrap();
        writeln!(f, "past content").unwrap();
        let past_time = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        filetime::set_file_mtime(
            &past_file,
            filetime::FileTime::from_system_time(past_time.into()),
        )
        .unwrap();

        let mid_file = dir.path().join("mid.txt");
        let mut f = File::create(&mid_file).unwrap();
        writeln!(f, "mid content").unwrap();
        let mid_time = Utc.with_ymd_and_hms(2023, 1, 1, 0, 0, 0).unwrap();
        filetime::set_file_mtime(
            &mid_file,
            filetime::FileTime::from_system_time(mid_time.into()),
        )
        .unwrap();

        let recent_file = dir.path().join("recent.txt");
        let mut f = File::create(&recent_file).unwrap();
        writeln!(f, "recent content").unwrap();
        let recent_time = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        filetime::set_file_mtime(
            &recent_file,
            filetime::FileTime::from_system_time(recent_time.into()),
        )
        .unwrap();

        // Test range: 2021 to 2025 (should only include mid.txt)
        let config = WalkerConfig::default()
            .with_newer_than(Some(
                Utc.with_ymd_and_hms(2021, 1, 1, 0, 0, 0).unwrap().into(),
            ))
            .with_older_than(Some(
                Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap().into(),
            ));

        let walker = Walker::new(dir.path(), config);
        let files: Vec<_> = walker.walk().filter_map(Result::ok).collect();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path.file_name().unwrap(), "mid.txt");
    }

    #[test]
    fn test_walker_multiple_regex_include() {
        use regex::Regex;
        let dir = create_test_dir();

        let config = WalkerConfig::default().with_regex_include(vec![
            Regex::new("file1").unwrap(),
            Regex::new("nested").unwrap(),
        ]);
        let walker = Walker::new(dir.path(), config);
        let files: Vec<_> = walker.walk().filter_map(Result::ok).collect();
        assert_eq!(files.len(), 2);
        let names: Vec<_> = files
            .iter()
            .map(|f| f.path.file_name().unwrap().to_str().unwrap())
            .collect();
        assert!(names.contains(&"file1.txt"));
        assert!(names.contains(&"nested.txt"));
    }

    #[test]
    fn test_walker_regex_filters() {
        use regex::Regex;
        let dir = create_test_dir();

        // create_test_dir creates: file1.txt, file2.txt, subdir/nested.txt

        // Test regex include
        let config = WalkerConfig::default().with_regex_include(vec![Regex::new("file1").unwrap()]);
        let walker = Walker::new(dir.path(), config);
        let files: Vec<_> = walker.walk().filter_map(Result::ok).collect();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path.file_name().unwrap(), "file1.txt");

        // Test regex exclude
        let config = WalkerConfig::default().with_regex_exclude(vec![Regex::new("file2").unwrap()]);
        let walker = Walker::new(dir.path(), config);
        let files: Vec<_> = walker.walk().filter_map(Result::ok).collect();
        // Should have file1.txt and nested.txt
        assert_eq!(files.len(), 2);
        let names: Vec<_> = files
            .iter()
            .map(|f| f.path.file_name().unwrap().to_str().unwrap())
            .collect();
        assert!(names.contains(&"file1.txt"));
        assert!(names.contains(&"nested.txt"));
        assert!(!names.contains(&"file2.txt"));

        // Test combined
        let config = WalkerConfig::default()
            .with_regex_include(vec![Regex::new("file").unwrap()])
            .with_regex_exclude(vec![Regex::new("1").unwrap()]);
        let walker = Walker::new(dir.path(), config);
        let files: Vec<_> = walker.walk().filter_map(Result::ok).collect();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path.file_name().unwrap(), "file2.txt");
    }

    #[test]
    fn test_walker_file_type_filters() {
        use super::super::FileCategory;
        let dir = TempDir::new().unwrap();

        // Create files with different extensions (ensure they are not empty)
        let image = dir.path().join("photo.jpg");
        let mut f = File::create(&image).unwrap();
        writeln!(f, "image content").unwrap();

        let doc = dir.path().join("report.pdf");
        let mut f = File::create(&doc).unwrap();
        writeln!(f, "document content").unwrap();

        let audio = dir.path().join("song.mp3");
        let mut f = File::create(&audio).unwrap();
        writeln!(f, "audio content").unwrap();

        // Test filtering for images
        let config = WalkerConfig::default().with_file_categories(vec![FileCategory::Images]);
        let walker = Walker::new(dir.path(), config);
        let files: Vec<_> = walker.walk().filter_map(Result::ok).collect();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path.file_name().unwrap(), "photo.jpg");

        // Test filtering for documents and audio
        let config = WalkerConfig::default()
            .with_file_categories(vec![FileCategory::Documents, FileCategory::Audio]);
        let walker = Walker::new(dir.path(), config);
        let files: Vec<_> = walker.walk().filter_map(Result::ok).collect();
        assert_eq!(files.len(), 2);
        let names: Vec<_> = files
            .iter()
            .map(|f| f.path.file_name().unwrap().to_str().unwrap())
            .collect();
        assert!(names.contains(&"report.pdf"));
        assert!(names.contains(&"song.mp3"));

        // Test with no match
        let config = WalkerConfig::default().with_file_categories(vec![FileCategory::Videos]);
        let walker = Walker::new(dir.path(), config);
        let files: Vec<_> = walker.walk().filter_map(Result::ok).collect();
        assert_eq!(files.len(), 0);
    }

    #[test]
    fn test_walker_shutdown_flag() {
        let dir = create_test_dir();

        // Create many files
        for i in 0..10 {
            let file = dir.path().join(format!("file{}.txt", i));
            let mut f = File::create(&file).unwrap();
            writeln!(f, "Content {}", i).unwrap();
        }

        let shutdown = Arc::new(AtomicBool::new(false));
        let walker = Walker::new(dir.path(), WalkerConfig::default())
            .with_shutdown_flag(Arc::clone(&shutdown));

        // Set shutdown flag immediately
        shutdown.store(true, Ordering::SeqCst);

        let files: Vec<_> = walker.walk().filter_map(Result::ok).collect();

        // With shutdown flag set, we should get very few or no files
        // (depending on timing, might get a few before the flag is checked)
        assert!(
            files.len() < 5,
            "Expected early termination, got {} files",
            files.len()
        );
    }

    #[test]
    fn test_walker_handles_nonexistent_path() {
        let walker = Walker::new(
            Path::new("/nonexistent/path/12345"),
            WalkerConfig::default(),
        );

        let results: Vec<_> = walker.walk().collect();

        // Should produce errors, not panic
        assert!(results.is_empty() || results.iter().all(|r| r.is_err()));
    }

    #[test]
    #[cfg(unix)]
    fn test_walker_detects_hardlinks() {
        use std::fs::hard_link;

        let dir = create_test_dir();

        // Create original file
        let original = dir.path().join("original.txt");
        let mut f = File::create(&original).unwrap();
        writeln!(f, "Original content").unwrap();

        // Create hardlink to original
        let hardlink = dir.path().join("hardlink.txt");
        hard_link(&original, &hardlink).unwrap();

        let walker = Walker::new(dir.path(), WalkerConfig::default());

        let files: Vec<_> = walker.walk().filter_map(Result::ok).collect();

        // Only one of the hardlinked files should appear
        let matching: Vec<_> = files
            .iter()
            .filter(|f| {
                f.path
                    .file_name()
                    .map_or(false, |n| n == "original.txt" || n == "hardlink.txt")
            })
            .collect();

        assert_eq!(
            matching.len(),
            1,
            "Only one hardlink file should be included"
        );
    }

    #[test]
    fn test_file_entry_fields() {
        let dir = create_test_dir();
        let walker = Walker::new(dir.path(), WalkerConfig::default());

        let file = walker.walk().filter_map(Result::ok).next().unwrap();

        // Verify all fields are populated
        assert!(!file.path.as_os_str().is_empty());
        assert!(file.size > 0);
        assert!(file.modified != SystemTime::UNIX_EPOCH);
        assert!(!file.is_symlink);
        // is_hardlink depends on whether we've seen the inode before
    }
}
